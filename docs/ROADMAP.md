# Roadmap

Feature parity with the reference product is reached. This page is the plan from here:
make it solid, ship it, take it to Windows and Linux, make working together easy without
a cloud, then go past the reference app. Local files only (decisions 1 and 2).

Refs: `D` entries in `docs/decisions.md`, behavior notes by number
(`docs/research/behaviors/`), `E-` rows in `docs/research/engine-checks.md`, feature IDs
in `docs/parity/features.csv`.
Size: **S** a day or two, **M** about a week, **L** more than a week.

## Waiting on the owner

Kept current by each roadmap PR (MOrg's loop, 2026-10-18). A weekly digest reads this
section. Everything else on this page goes ahead without waiting.

**Go-aheads**
- **v0.8.0's tag**: the owner said yes to starting it (2026-11-06), which covers this release
  PR and a rehearsal; the tag and publishing need their own yes, once the rehearsal is green.
  Files using the new keys won't open in 0.7.0. It is the first release the launcher can require
  a signature from.

**Needs the owner's hands or hardware**
- **The update and both-chips tests**, about five minutes on the Mac, now on the newest
  release: the in-app update from the version installed, both chips in the app, the Intel
  half opening under Rosetta (the one that needs the screen), and a checksum. The commands
  are in [research/release-0.3.0.md](research/release-0.3.0.md); use the newest version
  number in them.
- **Screen block 8**, the rest of block 7 (2026-11-04, cut short; results in
  [research/screen-block-7.md](research/screen-block-7.md)), on the owner's yes: the board
  saved as a PDF for K185 and K187 on paper (test files in `~/Documents`, so the save panel
  opens on an ordinary folder), the Layouts rename then click, crash recovery in the window
  (`scripts/crash-check.sh`, M2), and in the reference app E-S34 and the two report
  questions. Block 7 passed D219, the Tauri 2.12 window, menus and Save dialog, K184, M192,
  one-day crew calls, the Red Flag Types rename, and K185 and K187 on screen. Also a call sheet
  with a map picture chosen in the real Open dialog and saved as a PDF (D245).
- **M4's live run**, about fifteen minutes with two computers on one shared folder
  (any sync service): open the same `.mtsc` on both with the network off on one, change
  different things on each (and one thing on both), turn the network back on, and check
  that each window either offers Merge for the file on disk or shows "Conflict copy beside
  this file" in the footer, and that merging keeps every change or offers it as a pick.
- **A Linux machine, eventually**, for M3's hand checks there (the screen block 3 list),
  including the keys: every shortcut is Ctrl there, and a test keeps them off the keys the
  usual Linux desktops keep for themselves; pressing them is the live check. Also
  double-clicking a `.mtsc` and a `.mtst` (both associated in `tauri.conf.json`).

**Deferred (the owner, 2026-10-20)**: "defer windows tests indefinitely. We can do linux ones
eventually." Windows is still built and tested automatically (the weekly build and every
release make and test the Windows app and its installer), and its release notes keep
saying it hasn't been tested by a person yet. The Windows hand checks (the same list as
Linux's, plus SmartScreen on the unsigned installer) wait until the owner says otherwise.

**Decisions** (each with a recommendation; reversible ones are built on it)
- **Revision colors on strips** (D247): built as a small swatch at the start of the strip,
  so the I/E and day or night colors keep their meaning. Reversible.
- **Moving to the suite's shared updater** (M3): recommended as its own piece of work just
  before a release, not squeezed in. It changes how every release is signed and how every
  copy installs an update, so it needs a release-workflow review and a real update rehearsal.
- **The call sheet's map** (D245): built as a picture you choose for each location, with its
  address, directions and coordinates, because a map from a map service would mean going
  online. Recommended and reversible; say if you want a map service as an opt-in instead.
- **Merge in the browser host** (D226, D239): settled. The web-based conversion asked, so the host now
  takes the base schedule as a file beside theirs (`merge_preview`, `merge_apply`); the
  page in a browser still offers no merge picker of its own.
- **Unknown keys in a schedule file** (D224, D227): **approved by the owner** (2026-10-20,
  with the v0.4.0 go-ahead). The reader stays strict, and one reserved `extensions` object
  (at the top level and on the schedule, categories, red flag types, elements, sheets,
  boards and sub-boards) is kept and written back untouched. Additive: every existing file
  writes back byte for byte. Ships in 0.4.0.
- **The browser build** (D222): **decided by the owner** (2026-10-18), all five as
  recommended. The partner hosts the whole page; Method Budget's partner terms apply; a
  read-only viewer keeps views, Find, printing, the call sheet and exports, with edits
  refused; Method Budget's shim is copied now and moves to `method-libs` later; the `/try/`
  demo with Field Station comes last.
- Strip designer "scale to content" (D87, D185): **built on the recommendation, dropped**
  (D221). The reference app's "Scale height" changed nothing visible (screen block 6), and
  strips already size to their lines.

---

## Where we are (2026-10-18)

- **Parity** ([docs/parity/PARITY.md](parity/PARITY.md)): every v1 feature (108 of 108)
  and every later feature is built; 13 rows are skipped as out of scope. 99.4% of the
  tracked features the reference app has are checked in the real app (behavior notes 14
  to 27); 97.9% are checked live in our own app (screen blocks 4 and 5).
- **Tests**: 580 passing in `cargo test --workspace --locked`, with seeded property
  tests on the DOOD, merge (both orders alike, decision 174), find and replace, and the
  board, and fuzz tests on the file reader and the script readers (decisions 172 to 175),
  which a weekly job (`fuzz.yml`) runs at a hundred times the rounds. Release-mode speed
  budgets at 1,500 strips run in CI (`budgets`, decisions 176 to 178, 190, 191): an edit
  plus the board view takes about 1.3 ms on a laptop against a 5 ms budget.
- **What exists**: the engine, the open file format (`docs/FORMAT.md`), the command line,
  the MCP server (`docs/MCP.md`), the desktop app (`docs/APP.md`), diff and three-way
  merge with a git merge driver (`docs/MERGE.md`), outside changes settled in the app
  (decisions 179 to 182), moving strips between windows (decision 183), the budget
  exchange file at version 1
  (`docs/EXCHANGE.md`, decisions 201 to 203), call sheet data (`docs/CALLSHEET.md`) and the
  extras beyond the reference app (BYD-01 to BYD-12), a local signed Mac release
  script with a draft-release workflow (decision 196), the sample production and the
  user guide in the Help menu (decisions 198 and 199) and crash recovery as tests
  (decision 200). Decisions run to 221.
- **Not yet**: a notarized build anyone can download, Windows and Linux run by hand, the
  checks screen block 6 didn't run, and the rows left below. The site is live and the
  repository public (decisions 209, 216).

Parity upkeep: edit `docs/parity/features.csv` when a feature or a check lands, then run
`python3 scripts/parity.py --snapshot` (docs/parity/README.md).

---

## M1 Hardening

**Goal**: nothing a production relies on breaks, loses data or slows down, before anyone
outside downloads it.

| Item | Size | Refs |
|---|---|---|
| Open engine check still to see in the reference app: whether an element left off the board counts in a category's element count (E-S34; E-L5 is settled, screen block 7); every other queued check was answered in screen block 4 (behavior note 27) | S | engine-checks.md queue |
| Screen block 8: what block 7 (2026-11-04, cut short; screen-block-7.md) didn't run: K185 and K187 on paper; the Layouts rename then click; and in the reference app E-S34, and whether its spreadsheets follow a report's filters and without holds and whether a cast member off the board gets a name with Cast names on (RPT-03, RPT-10). Block 7 passed D219, K184, M192, the Red Flag Types rename, K185 and K187 on screen, and E-L5; block 6 (2026-10-17) passed G167 to G171, J179 to J182, K183, K186, K188, M193, M194 and the new screens | S | screen-block-7.md, screen-block-6.md |
| Live checks still open: DOD-06 and DOD-07 in the window, BYD-10 (behavior note 27's small items passed in screen block 6) | S | behavior note 27 |

**Exit**: every review finding fixed or written up as a decision; no open engine check
without a rule and a test; the budgets and fuzz targets run in CI with no failures for a
week of runs.

---

## M2 First release (alpha)

**Goal**: a production can download a Mac app, open a sample, and trust it with a real
schedule.

| Item | Size | Refs |
|---|---|---|
| Crash recovery in the real window: run `scripts/crash-check.sh` in a screen block (the saving code is tested, D200) | S | APP.md "Saving and recovery" |

**Exit**: a signed build installs on a clean Mac without warnings, updates itself to the
next build, and the site links to it.

---

## M2b The browser build

**Goal**: the window-free session and the WebAssembly build Method Budget has, so a
partner's web app can host Method Schedule's page with the engine in the browser (the
owner, 2026-10-18; D222). The plan, the crate map and every command by kind are in
[research/web-session.md](research/web-session.md). Ahead of M3; the owner can reorder.

| Item | Size | Refs |
|---|---|---|

Done: the web smoke test in the weekly build (2026-11-06; its first run is the next Monday's).

Done: the plan brought up to date (wasm check, seams, every command mapped), and the
crate itself with Method Budget's injected clock and `cargo xtask wasm-check` in CI, which
also fails if `getrandom` appears under the session. Then the core of the session moved:
`session`, `board`, `sheet`, `elements`, `calendar`, `view`, `clipboard`, `conflicts`,
`conflictmarks`, `explain` and `redflags`, with the file-free half of `safety` (the disk
stamp's data, recovery keys, the autosave rule on the clock). The desktop keeps reading and
writing files (`SessionFiles`, `StampAt`, the image reader and picker) and installs the
system clock at startup.

Also done: #26's review points. `Session::with_schedule` asserts `Schedule::check` in
debug builds (every caller hands in a checked schedule); `mark_saved` and `restamp` have
their contract written down, asserted and tested in the session crate; and the board and
view tests that had stayed on the desktop now run in the session crate, the desktop keeping
only their report and export halves.

Phase 1 done (2026-10-20): every window-free module is in the session crate, the last being
the logic of `plan` (timelines, locations, rest, locks and drift, pins, the order suggestion,
call sheet data). `diffmerge` stays on the desktop: it is the file side (reading both files,
the backups and their times) around the engine's own merge, which the session already
reaches, and sharing in the browser is the partner's server's (decision 222). `docs`, the
open-schedules list, is window bookkeeping and stays too.

**Complete** (2026-10-20). Phases 1b, 2 and 3 are in: the browser host runs every command
that needs only the schedule, plus files in and out, the picture store, preferences and the
clipboard; `method-schedule-web` builds the page with the engine as WebAssembly
(`cargo xtask web`, its smoke test passing); the browser draws the desktop's menus; and the
`/try/` demo (`cargo xtask web --demo`) is hardened and linked from the site and the feature
list for 0.5.0, when the website's sync builds it. The web refactor that closed it: every
command has one body, in `method_schedule_session::commands`, which the desktop's Tauri
commands and the browser host both call (#90), and the file commands share their
file-free halves (#91), so the generated copy of the desktop's bodies and its script are
gone.

Not in this milestone: moving the CLI and MCP server onto the session (Method Budget's
cli-through-session); both keep working as they are.

**Exit**: the page runs in a browser on the engine built to WebAssembly, the desktop is
unchanged, and CI checks both.

---

## M2c The Field Station showcase

**Goal**: the app opens a rich invented sample that shows every feature (SUITE-STANDARDS.md
section 8, the owner, 2026-10-18), and it matches Method Budget's sample through the real
exchange file. Alongside M2b. Nothing left on it.

Done: pictures on the park truck, the quad bike, the hand radio and the paper map, drawn in
code by `scripts/make-sample-pictures.py` (not generated), kept in `fixtures/images/` beside
the fixture and carried in the app for the untitled sample (2026-10-20). Help > Open Sample
opens Field Station, untitled, and the first launch with nothing to open shows it once
(2026-10-19). Earlier (decision 223): the shared shape agreed with Method Budget; Storyteller's 90-scene
script, our first 11 scenes kept; Field Station rebuilt from it with the real importer
(18 days, three locations, a second unit, nine cast with one silent, an element in every
category, background counts, banners, call times, red flags, prep 2027-04-05 to wrap
2027-06-09); its exchange file in Method Budget's $1M sample (`days_hash` e9245ad76e787b1e);
every picture retaken.

---

## M3 Windows and Linux

**Goal**: the same app on Windows (WebView2) and Linux (WebKitGTK): built and tested
automatically on both, and by hand on Linux eventually. Windows hand tests are deferred
(the owner, 2026-10-20; Waiting on the owner).

| Item | Size | Refs |
|---|---|---|
| Printing and Save as PDF without the dialog on both; PDF document properties (D96) checked | M | APP.md "Not yet" |
| Image types (TIFF, HEIC, BMP) as each web view shows them | S | D104, IMP-09 |
| Single instance on Windows and Linux, and the system recent documents list. Waiting on a dependency: tauri-plugin-single-instance reaches D-Bus on Linux through zbus crates RustSec lists as unmaintained (RUSTSEC-2026-0370, 0371, 0372), which `cargo deny` refuses; once the plugin moves off them, which is what's left: tried on 2026-11-06, a Windows-only dependency and targets in `deny.toml` still fail on zvariant, zvariant_derive and zbus_names, and add about 430 lines to `Cargo.lock` for a feature with no Windows hand test (deferred). Wait for the plugin to move off them. The handler (a second launch's file to `on_opened`) is small | S | APP.md "Not yet" |
| The suite's shared updater: replace `tauri-plugin-updater`'s install and `updater.rs`'s policy code with `method_update` from `method-libs` (pinned by full commit, `method-libs` added to the crate-age git allow list), keeping our Help menu and dialogs; after 0.1.0 ships. The crate's README has the migration notes. Also carries the release-workflow fix that compiles before any signing key is unpacked (local branch `ci/compile-before-secrets`) | M | D213, D214, SUITE-STANDARDS.md section 1 |

**Exit**: the live check list from screen block 3 passes on Linux, and both systems'
installers build, pass their automatic tests and update themselves (Windows hand checks
deferred, the owner, 2026-10-20).

---

## M4 Collaboration without a cloud

**Goal**: two people on one schedule through a shared folder or git, with nothing lost.

**Exit**: two copies edited apart in a shared folder and in git merge with every change
kept or offered as a pick, in tests and in a live run.

Done: going to a change from inside the merge, the picks kept while it waits (D240,
2026-10-30); shared folders: the file on disk merged against the version last read (D180)
and conflict copies beside the file offered for merging (D241, 2026-11-02). Git merges
through the CLI's `merge-driver` and `git-setup`. What is left is the exit's live run
(Waiting on the owner).

---

## M5 Beyond the reference app

**Goal**: the things users say the reference app never did (behavior note 13).

| Item | Size | Refs |
|---|---|---|

Done: revision colors on strips and renaming a draft's color (D247, 2026-11-06). The Method
Budget link: the exchange frozen at version 1 (2026-10-15), Method Budget's reader built, Export
for Budget in the app. The family strip: the suite website's bar and band carry it
(WEBSITE.md).

Done: auto-schedule improvements (D246, 2026-11-06): weights people can set, a day's length in
the cost, and a search by swapping scenes beyond the three orders.

Done: call sheets beyond the printed sheet (decision 207): the advance schedule (D232), crew
calls by department (D242), for one day only (D243), and directions with a map picture for
each location (D245, 2026-11-06). Weather would need a service and stays out with
distribution (BYD-03).

Done: per-person call times (D231, 2026-10-21). The Day Timeline sets each person's own
call and wrap; the rest checks and the call sheet use them. The call sheet's advance
schedule, the next two shoot days in brief (D232, 2026-10-21). Script drafts: each merged
revision's color, date and changes, under Plan > Script Drafts (D233, 2026-10-22). Suggest Strip Order compares three orders
by one cost and keeps the board's when none is cheaper (D234, 2026-10-26). Crew calls by
department, each against the company's call, on the call sheet (D242, 2026-11-03), and a
department's call for one day only, set in the Day Timeline (D243, 2026-11-04). Clocks set
by a lunar calendar or a decree: a location's periods of its own offset, typed from the
year's announcement (D244, 2026-11-05).

**Exit**: each item ships behind its own menu entry or option, documented, with the
exchange at version 1 read by Method Budget.

---

## Decisions settled

Open decisions live in "Waiting on the owner" at the top.

Settled and dropped from this table: Fountain lines per page, 56 (7 lines an eighth) with
a setting to change it (D195); signing the Mac build with the company's Developer ID, and
Windows unsigned for the first alpha with the SmartScreen warning accepted (D196); the
exchange file frozen at version 1 after Method Budget's first build reads it, and the
repository going public, the website, Discussions and the support email deferred, the
updater key made then (D197) (all the owner's answers of 2026-10-15); the name, extension and domain (D27), the brand
family (BRAND.md: shared voice and neutrals, own accent), the exchange file as the
contract (EXCHANGE.md, agreed 2026-10-11), one drop per element (D10), new elements with
drops off and a gap of 1 (D10), the worked total (Total = Work + Hold, D11 and D117), the
Universal Schedule Standard (D92), and the extras built opt-in (D68 to D75).
