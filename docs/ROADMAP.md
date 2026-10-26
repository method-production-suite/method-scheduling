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
- **v0.6.0 on 4892e3e, asked through MOrg on 2026-10-22**: per-person call times feeding
  the rest checks (D231), the call sheet's advance schedule (D232) and script drafts with
  their colors and dates (D233). Release PR #104 merged; the rehearsal on 4892e3e passed
  (notarized, Gatekeeper accepted, universal). For the
  notes: files using per-person call times or drafts (`plan.person_calls`,
  `plan.drafts`) won't open in 0.5.0 or earlier. v0.5.0 was published on 2026-10-21 and
  methodschedule.com follows it with the demo.

**Needs the owner's hands or hardware**
- **The update and both-chips tests**, about five minutes on the Mac, now on the newest
  release: the in-app update from the version installed, both chips in the app, the Intel
  half opening under Rosetta (the one that needs the screen), and a checksum. The commands
  are in [research/release-0.3.0.md](research/release-0.3.0.md); use the newest version
  number in them.
- **Screen block 7**, one block on the owner's screen, asked for through MOrg (the suite
  rule since 2026-10-18; everything else here is tested offscreen): a file opened from the Finder opens in its
  own window (D219); K184 Find with the board beside the board; M192 the Day Timeline past
  wrap; Layouts and Red Flag Types rename then click; K185 and K187 on paper; crash
  recovery in the window (`scripts/crash-check.sh`, M2); the app on Tauri 2.12 opening, its
  menus and a Save dialog (the move's live check; Linux too, eventually); and in the reference app E-L5,
  E-S34 and the two report questions (M1's review row).
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
- **Merge in the browser host** (D226), built on the recommendation: the browser host offers
  Revise and What Changed but not Merge, which needs a common version from backups a browser
  doesn't keep. Reverse it and the host takes the base schedule as a third file.
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
| Open engine checks still to see in the reference app: whether a fixed built-in category set exists beyond the cast (E-L5), and whether an element left off the board counts in a category's element count (E-S34); every other queued check was answered in screen block 4 (behavior note 27) | S | engine-checks.md queue |
| Screen block 7: a file opened from the Finder opens in its own window (decision 219); and what block 6 didn't run (docs/research/screen-block-6.md, "Not run"): K184 Find with the board beside the board; M192 Day Timeline past wrap on a day with shoot times; Layouts and Red Flag Types rename then click; K185 and K187 on paper; and in the reference app E-L5 and E-S34, and whether its spreadsheets follow a report's filters and without holds and whether a cast member off the board gets a name with Cast names on (RPT-03, RPT-10). Block 6 (2026-10-17) passed G167 to G171, J179 to J182, K183, K186, K188, M193, M194 and the new screens; its four look bugs are fixed (decision 218) | S | screen-block-6.md |
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
| A signed and notarized `.app` and `.dmg`: the company's Developer ID is on the owner's Mac and `scripts/release-mac.sh` builds, signs and verifies (D196); notarizing waits for the owner's one-time `notarytool store-credentials method-schedule-notary` profile | S | D196, APP.md "Updates and releases" |
| The auto-updater: built, with the owner's public key (D213, D214); still the owner's: `TAURI_SIGNING_PRIVATE_KEY` and its password in the repository's secrets, the repository public, and a first published release | S | D197, D213, D214, APP.md |
| The release workflow: `release.yml` drafts a release with unsigned builds for all three systems on a tag, signing steps skipped until secrets exist (D196); still to add: the update manifest, and signing secrets when the owner wants CI to sign | S | `.github/workflows/release.yml` |
| Release notes on the site's release page, written from the decisions since the last tag | S | WEBSITE.md |
| The website: live on methodschedule.com through the suite website (decision 209); the repository public with Discussions (decision 216), help through GitHub with no email address of our own (WEBSITE.md), the security policy from the organization's `.github` repository: done | S | WEBSITE.md |
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
| The web smoke test in CI: `cargo xtask web --smoke` in the weekly build, after the process freeze ends on 2026-11-02 (CI changes wait until then) | S | phase 2 |

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
gone. Left, waiting on the process freeze (2026-11-02): the smoke test in the weekly build.

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
| Builds and installers: the NSIS setup on Windows, AppImage and `.deb` on Linux, under version-free names (built unsigned by `release.yml`, D196, D210); Windows ships unsigned for the first alpha, SmartScreen warning accepted | M | D196, D210 |
| CI: Linux app builds in the weekly full build (`platforms.yml` builds and tests all three systems every Monday since 2026-10-19, ArtCraft-style; release.yml builds the installers) | S | `.github/workflows/platforms.yml` |
| Printing and Save as PDF without the dialog on both; PDF document properties (D96) checked | M | APP.md "Not yet" |
| Image types (TIFF, HEIC, BMP) as each web view shows them | S | D104, IMP-09 |
| Single instance on Windows and Linux, and the system recent documents list. Waiting on a dependency: tauri-plugin-single-instance reaches D-Bus on Linux through zbus crates RustSec lists as unmaintained (RUSTSEC-2026-0370, 0371, 0372), which `cargo deny` refuses; once the plugin moves off them, or after the process freeze `deny.toml` checks per target and the plugin goes in for Windows alone. The handler (a second launch's file to `on_opened`) is small | S | APP.md "Not yet" |
| The suite's shared updater: replace `tauri-plugin-updater`'s install and `updater.rs`'s policy code with `method_update` from `method-libs` (pinned by full commit, `method-libs` added to the crate-age git allow list), keeping our Help menu and dialogs; after 0.1.0 ships. The crate's README has the migration notes. Also carries the release-workflow fix that compiles before any signing key is unpacked (local branch `ci/compile-before-secrets`) | M | D213, D214, SUITE-STANDARDS.md section 1 |

**Exit**: the live check list from screen block 3 passes on Linux, and both systems'
installers build, pass their automatic tests and update themselves (Windows hand checks
deferred, the owner, 2026-10-20).

---

## M4 Collaboration without a cloud

**Goal**: two people on one schedule through a shared folder or git, with nothing lost.

| Item | Size | Refs |
|---|---|---|
| Merge UI polish: a click on a change goes to its sheet, element, strip or date; each conflict shown side by side | M | D26, APP.md "Not yet" |
| Conflict review: a list of every conflict pick, kept with the file until resolved | M | MERGE.md |
| Shared folders (iCloud Drive, Dropbox, network drives): the outside-change guard settled by merging with the file on disk, keeping the version last read as the base | M | APP.md "Saving and recovery" |
| A rename on one side matched to the same element on the other | S | D25 note |

**Exit**: two copies edited apart in a shared folder and in git merge with every change
kept or offered as a pick, in tests and in a live run.

---

## M5 Beyond the reference app

**Goal**: the things users say the reference app never did (behavior note 13).

| Item | Size | Refs |
|---|---|---|
| Call sheets beyond the printed sheet (decision 207): crew calls and a map (the advance schedule came in D232); distribution stays out (BYD-03) | M | CALLSHEET.md, BYD-02 |
| Daylight saving rules beyond the US and Canada's, the EU's, Australia's south-east, New Zealand's, Israel's, Egypt's and mainland Chile's (D230, 2026-10-21): the ones that move with a lunar calendar or by decree (others keep a typed offset today) | S | D72, D193, D220, D230, BYD-08 |
| Auto-schedule improvements: weights people can set, searching beyond the three orders, a day's length in the cost (a cost and three orders since D234; a long day's tail moves to the next day by hand since D192) | M | D75, D234, BYD-06 |
| Script revision workflow: revision colors on strips (needs a design beside the I/E and day or night colors), picking or renaming a color (each draft's color, date and changes are kept, shown on the Breakdown tab and printed since D233) | M | D69, D233, BYD-01 |
| Method Budget live link: freeze the exchange at version 1 once Method Budget's first build reads it (owner, 2026-10-15; worked out with Method Budget), then Export for Budget in the app | M | EXCHANGE.md, D197 |
| Method Production suite site and the family strip, once methodproduction.com is live | S | WEBSITE.md "The family" |

Done: per-person call times (D231, 2026-10-21). The Day Timeline sets each person's own
call and wrap; the rest checks and the call sheet use them. The call sheet's advance
schedule, the next two shoot days in brief (D232, 2026-10-21). Script drafts: each merged
revision's color, date and changes, under Plan > Script Drafts (D233, 2026-10-22). Suggest Strip Order compares three orders
by one cost and keeps the board's when none is cheaper (D234, 2026-10-26).

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
