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
- **Check and publish the v0.1.0 draft.** The secrets are set and v0.1.0 is tagged on the
  release PR's merge (2026-10-18). Install the `.dmg` on an Apple silicon Mac, open Field
  Station, confirm the update archive and `latest.json` are attached, then publish. The
  site syncs from the release once it's published.

**Needs the owner's hands or hardware**
- **Screen block 7**, one unattended block: a file opened from the Finder opens in its
  own window (D219); K184 Find with the board beside the board; M192 the Day Timeline past
  wrap; Layouts and Red Flag Types rename then click; K185 and K187 on paper; crash
  recovery in the window (`scripts/crash-check.sh`, M2); and in the reference app E-L5,
  E-S34 and the two report questions (M1's review row).
- **A tagged script in the .fdx format** for IMP-04, made in the app that writes it.
- **Windows and Linux machines** for M3's hand checks (the screen block 3 list).

**Decisions** (each with a recommendation; reversible ones are built on it)
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
| Live checks still open: IMP-04 (needs a tagged Final Draft script), DOD-06 and DOD-07 in the window, BYD-10 (behavior note 27's small items passed in screen block 6) | S | behavior note 27 |

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
| Hardening from #26's review: `with_schedule` runs `Schedule::check` (or says why not), the `mark_saved`/`restamp` contract written down and tested in the session crate, and `moved_tests.rs` split so the session crate tests its own code | S | #26 |
| `method-schedule-session`, the rest of the window-free modules: `find`, `events`, `breakdown`, `flagcolors`, `split`, the data side of `reports`, `reportsetup`, `layouts`, `plan` and `diffmerge`, and `docs` | M | web-session.md phase 1 |
| `Host::dispatch`, `COMMANDS`, `COMMANDS_VERSION`; files in and out as base64; the picture store; `host_info`; the read-only switch | M | phase 1b |
| `method-schedule-web`: the exports, the page's invoke shim, the page's `host_info` guards, `cargo xtask web`, a headless smoke test in CI | M | phase 2 |
| The `/try/` demo with Field Station | S | phase 3, WEBSITE.md |

Done: the plan brought up to date (wasm check, seams, every command mapped), and the
crate itself with Method Budget's injected clock and `cargo xtask wasm-check` in CI, which
also fails if `getrandom` appears under the session. Then the core of the session moved:
`session`, `board`, `sheet`, `elements`, `calendar`, `view`, `clipboard`, `conflicts`,
`conflictmarks`, `explain` and `redflags`, with the file-free half of `safety` (the disk
stamp's data, recovery keys, the autosave rule on the clock). The desktop keeps reading and
writing files (`SessionFiles`, `StampAt`, the image reader and picker) and installs the
system clock at startup.

Not in this milestone: moving the CLI and MCP server onto the session (Method Budget's
cli-through-session); both keep working as they are.

**Exit**: the page runs in a browser on the engine built to WebAssembly, the desktop is
unchanged, and CI checks both.

---

## M2c The Field Station showcase

**Goal**: the app opens a rich invented sample that shows every feature (SUITE-STANDARDS.md
section 8, the owner, 2026-10-18), and it matches Method Budget's sample through the real
exchange file. Alongside M2b.

| Item | Size | Refs |
|---|---|---|
| Pictures on elements: a few props and vehicles, drawn (not generated), kept beside the fixture | S | D223 |
| Help > Open Sample opens Field Station (today: Saltmarsh Road) as an untitled copy marked Sample, never the bundled file | S | help.rs |
| First launch with nothing to open: Field Station as that untitled copy, with a quiet line saying so; a setting to turn it off | S | safety_app.rs, reportsetup.rs |

Done (decision 223): the shared shape agreed with Method Budget; Storyteller's 90-scene
script, our first 11 scenes kept; Field Station rebuilt from it with the real importer
(18 days, three locations, a second unit, nine cast with one silent, an element in every
category, background counts, banners, call times, red flags, prep 2027-04-05 to wrap
2027-06-09); its exchange file in Method Budget's $1M sample (`days_hash` e9245ad76e787b1e);
every picture retaken.

---

## M3 Windows and Linux

**Goal**: the same app, tested by hand, on Windows (WebView2) and Linux (WebKitGTK).

| Item | Size | Refs |
|---|---|---|
| Builds and installers: the NSIS setup on Windows, AppImage and `.deb` on Linux, under version-free names (built unsigned by `release.yml`, D196, D210); Windows ships unsigned for the first alpha, SmartScreen warning accepted | M | D196, D210 |
| CI: Linux app builds in the nightly (`platforms.yml` tests all three systems nightly since 2026-10-18; release.yml builds the installers) | S | `.github/workflows/platforms.yml` |
| Printing and Save as PDF without the dialog on both; PDF document properties (D96) checked | M | APP.md "Not yet" |
| Image types (TIFF, HEIC, BMP) as each web view shows them | S | D104, IMP-09 |
| Keys: the reference app's menu keys mapped to Ctrl, conflicts with system keys | S | D79, D106 |
| Single instance handling and the system recent documents list | S | APP.md "Not yet" |
| File associations for `.mtsc` and `.mtst` | S | |
| The suite's shared updater: replace `tauri-plugin-updater`'s install and `updater.rs`'s policy code with `method_update` from `method-libs` (pinned by full commit, `method-libs` added to the crate-age git allow list), keeping our Help menu and dialogs; after 0.1.0 ships. The crate's README has the migration notes. Also carries the release-workflow fix that compiles before any signing key is unpacked (local branch `ci/compile-before-secrets`) | M | D213, D214, SUITE-STANDARDS.md section 1 |

**Exit**: the live check list from screen block 3 passes on both systems, and their
installers update themselves.

---

## M4 Collaboration without a cloud

**Goal**: two people on one schedule through a shared folder or git, with nothing lost.

| Item | Size | Refs |
|---|---|---|
| Merge UI polish: a click on a change goes to its sheet, element, strip or date; each conflict shown side by side | M | D26, APP.md "Not yet" |
| What Changed against a chosen backup; any file as a merge base | S | D26 |
| Conflict review: a list of every conflict pick, kept with the file until resolved | M | MERGE.md |
| The git merge driver documented for users, with a setup command | S | D25, MERGE.md |
| Shared folders (iCloud Drive, Dropbox, network drives): the outside-change guard settled by merging with the file on disk, keeping the version last read as the base | M | APP.md "Saving and recovery" |
| A rename on one side matched to the same element on the other | S | D25 note |

**Exit**: two copies edited apart in a shared folder and in git merge with every change
kept or offered as a pick, in tests and in a live run.

---

## M5 Beyond the reference app

**Goal**: the things users say the reference app never did (behavior note 13).

| Item | Size | Refs |
|---|---|---|
| Call sheets beyond the printed sheet (decision 207): crew calls, an advance schedule for the next days, a map; distribution stays out (BYD-03) | M | CALLSHEET.md, BYD-02 |
| Try it in your browser: the engine as WebAssembly at `/try/` with an invented schedule | L | WEBSITE.md "Try it in your browser, later" |
| Daylight saving rules beyond the US and Canada's, the EU's, Australia's south-east and New Zealand's (others keep a typed offset today) | S | D72, D193, D220, BYD-08 |
| Per-person call times, feeding the rest checks | M | D71, D73, BYD-09 |
| Schedule lock reports: drift per actor as a printed report and a spreadsheet | S | D74, BYD-05 |
| Auto-schedule improvements: more grouping rules, a cost to compare orders (a long day's tail moves to the next day by hand since D192) | M | D75, BYD-06 |
| Script revision workflow: revision colors and dates, a report of what each draft changed | M | D69, BYD-01 |
| Method Budget live link: freeze the exchange at version 1 once Method Budget's first build reads it (owner, 2026-10-15; worked out with Method Budget), then Export for Budget in the app | M | EXCHANGE.md, D197 |
| Method Production suite site and the family strip, once methodproduction.com is live | S | WEBSITE.md "The family" |

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
