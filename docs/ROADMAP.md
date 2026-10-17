# Roadmap

Feature parity with the reference product is reached. This page is the plan from here:
make it solid, ship it, take it to Windows and Linux, make working together easy without
a cloud, then go past the reference app. Local files only (decisions 1 and 2).

Refs: `D` entries in `docs/decisions.md`, behavior notes by number
(`docs/research/behaviors/`), `E-` rows in `docs/research/engine-checks.md`, feature IDs
in `docs/parity/features.csv`.
Size: **S** a day or two, **M** about a week, **L** more than a week.

---

## Where we are (2026-10-15)

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
  (decision 200). Decisions run to 217.
- **Not yet**: a notarized build anyone can download, a live site, Windows and Linux run
  by hand, M1's new UI seen in the real window (screen block 6), and the rows left below.

Parity upkeep: edit `docs/parity/features.csv` when a feature or a check lands, then run
`python3 scripts/parity.py --snapshot` (docs/parity/README.md).

---

## M1 Hardening

**Goal**: nothing a production relies on breaks, loses data or slows down, before anyone
outside downloads it.

| Item | Size | Refs |
|---|---|---|
| Open engine checks still to see in the reference app: whether a fixed built-in category set exists beyond the cast (E-L5), and whether an element left off the board counts in a category's element count (E-S34); every other queued check was answered in screen block 4 (behavior note 27) | S | engine-checks.md queue |
| Known gap from decisions: strip designer scale to content (needs a look at the reference app's style panel; D87, D185) | S | decisions.md |
| Review, DOOD and reports: in screen block 6, look at whether the reference app's spreadsheets follow a report's filters and without holds (they ignore its date range, note 27, as ours, D59), and whether a cast member off the board has a name with Cast names on (ours none, D142) | S | feature-review-2026-10-13.md, RPT-03, RPT-10 |
| Screen block 6 (one approval, display kept awake): see M1's new UI in the real window: G 167: day break sunrise/sunset from a located sheet (ZTEST LOT on scene 4) shows on the board.; G 168: calendar filter drop-down closes on Escape.; G 169: replace box reads "Replace with", tooltip has the rest.; G 170: Locations: type degrees + offset, Return, one click on Close closes. Layouts and Red Flag Types rename then click.; G 171: Export Call Sheet Data save dialog opens in the schedule's folder.; J 179-182: edit ZTEST-Lantern.mtsc outside (a copy written over it by shell) while open, then focus the window: question appears; Load Theirs, Undo; edit in app + outside change: Merge; Keep Mine; Later pauses autosave. File > Revert to Backup lists backups, picks one, Undo.; K 183: two windows (ZTEST-Lantern and big.mtsc copy): copy strips in one, Paste turns on in the other, paste brings sheets and elements; one Undo removes them.; K 184: Find with the board beside the board: marks on both.; K 185: Strip Layout | and _ buttons draw lines on screen and print.; K 186: Breakdown Designer: drag a field chip to another row; Breakdown tab follows.; K 187: banner font Serif/Monospaced on board and print.; K 188: printed board density (PDF) vs screen.; M 192: Day Timeline on a long day lists what runs past wrap; Move to Day N moves it, Undo puts it back.; M 193: Locations daylight saving select (None, US and Canada, EU) sets at once on a placed row; the day break's sun follows.; M 194: Strip Layout Category elements: pick Props, Board IDs on and off; board, print and one-line report.; MMS: the reference app's banner font list and strip designer "scale to content" (look only). | S | decisions 167 to 194 |
| Live checks still open: IMP-04 (needs a tagged Final Draft script), DOD-06 and DOD-07 in the window, BYD-10; and the small items at the end of behavior note 27 (day break sun times from locations, the calendar filter's Escape, the replace box's placeholder, the Locations Close button, the call sheet's save folder) | S | behavior note 27 |

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
| The website: live on methodschedule.com through the suite website (decision 209); still to do: make the repository public, turn on Discussions, the support email, a security policy | S | WEBSITE.md "For the owner: still to do" |
| Crash recovery in the real window: run `scripts/crash-check.sh` in a screen block (the saving code is tested, D200) | S | APP.md "Saving and recovery" |

**Exit**: a signed build installs on a clean Mac without warnings, updates itself to the
next build, and the site links to it.

---

## M3 Windows and Linux

**Goal**: the same app, tested by hand, on Windows (WebView2) and Linux (WebKitGTK).

| Item | Size | Refs |
|---|---|---|
| Builds and installers: the NSIS setup on Windows, AppImage and `.deb` on Linux, under version-free names (built unsigned by `release.yml`, D196, D210); Windows ships unsigned for the first alpha, SmartScreen warning accepted | M | D196, D210 |
| CI: run `platforms.yml` on every release tag as well as weekly, and add Linux app builds | S | `.github/workflows/platforms.yml` |
| Printing and Save as PDF without the dialog on both; PDF document properties (D96) checked | M | APP.md "Not yet" |
| Image types (TIFF, HEIC, BMP) as each web view shows them | S | D104, IMP-09 |
| Keys: the reference app's menu keys mapped to Ctrl, conflicts with system keys | S | D79, D106 |
| Single instance handling and the system recent documents list | S | APP.md "Not yet" |
| File associations for `.mtsc` and `.mtst` | S | |

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
| Daylight saving rules beyond the US and Canada's and the EU's (others keep a typed offset today) | S | D72, D193, BYD-08 |
| Per-person call times, feeding the rest checks | M | D71, D73, BYD-09 |
| Schedule lock reports: drift per actor as a printed report and a spreadsheet | S | D74, BYD-05 |
| Auto-schedule improvements: more grouping rules, a cost to compare orders (a long day's tail moves to the next day by hand since D192) | M | D75, BYD-06 |
| Script revision workflow: revision colors and dates, a report of what each draft changed | M | D69, BYD-01 |
| Method Budget live link: freeze the exchange at version 1 once Method Budget's first build reads it (owner, 2026-10-15; worked out with Method Budget), then Export for Budget in the app | M | EXCHANGE.md, D197 |
| Method Production suite site and the family strip, once methodproduction.com is live | S | WEBSITE.md "The family" |

**Exit**: each item ships behind its own menu entry or option, documented, with the
exchange at version 1 read by Method Budget.

---

## Decisions needed from the owner

None open (2026-10-15).

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
