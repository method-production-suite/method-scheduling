# Method Schedule user guide

A short guide for the person who schedules the film. It covers what you need on the first
day and where to look after that. The app shows this same guide under Help > User Guide.

## Getting started

Choose **Help > Open Sample**. It opens *Field Station*, an invented feature: 90 scenes,
nine cast (one of them never speaks), 18 shoot days at three locations with a company move,
a second unit for a day, something in every breakdown category, banners, call times and
red flags. The first time the app starts with nothing to open, it shows you this sample on
its own. It opens as a new, untitled schedule, so nothing you try in it changes what the
app ships. Move strips, break days
again, look at the day out of days, print a report. When you want to keep it, Save asks
where.

To start your own:

- **File > New** gives an empty schedule with the usual categories (Cast, Background,
  Stunts, Hand props and the rest).
- **File > Import Script** reads a Fountain (`.fountain`) or `.fdx` script, or a script
  scenes file (`.mscenes`, the format Method Storyteller's coming export writes): one
  breakdown sheet per scene, its heading split into I/E, set and time of day, its length in
  eighths and the speaking characters as cast. The scenes land in the Boneyard. A script scenes file brings its own
  exact pages and lengths, story days, the characters who don't speak, the episode and act,
  and the scenes the script omits.
- **File > New from Template** starts from the setup of an earlier show (categories,
  elements, layouts, report list) without its scenes.

Set a start date on the Board tab, move the scenes out of the Boneyard onto the board,
and break them into days. Everything else (dates, the day out of days, reports) follows.

## Coming from another scheduling app

Most of what you know carries over. The words are the industry's, so they mean what you
expect:

- **Breakdown sheets** are the Breakdown tab: one sheet at a time, its header fields at
  the top and its elements in a box per category.
- **The stripboard** is the Board tab: strips, day breaks and banners, with the Boneyard
  for scenes not on the schedule. A board can have several **sub-boards** (units), each
  with its own start date and calendar.
- **Elements and categories** live on the Elements tab, with board IDs, numbering, links
  and merging.
- **The day out of days** is its own tab, with the usual codes (`SW`, `W`, `WF`, `H`,
  `WD` and `PW` around a drop, `TR` for travel, `FT` for a fitting), Explain this count
  beside it, and your own code letters under Codes.
- **Calendars** are the Calendar tab: days off each week, holidays, company travel days
  and events.
- **Reports** are the Reports tab: one-line, shooting schedule, day out of days, element
  lists, breakdowns, calendars and more, each printable and saved as PDF.

What is different:

- Method Schedule doesn't open the other app's files. Start from the script, or from a
  template of your own. Your schedule is a plain `.mtsc` file you can read, diff and keep
  in any folder.
- **Autosave is on** by default. With a file, your changes are saved a few seconds after
  you stop. Turn it off under File > Autosave if you prefer to save by hand.
- Several windows, each its own schedule, with one clipboard between them.
- Keys follow the usual ones where they don't clash with the system's. Help > Keyboard
  Shortcuts lists every one.

## The breakdown

Open a sheet by double-clicking its strip on the board, or pick the Breakdown tab. The
header fields (scene, I/E, set, time of day, pages, location, script day, unit and the
rest) sit in three tight rows. Pages are typed in eighths the usual way: `1 3/8`, `4/8`,
or `2` for two pages.

Each category is a small box. Type a name in a box to add an element; the names already in
the schedule are offered as you type. Cut, copy and paste move elements between sheets.
Previous and Next (`⌘[` and `⌘]`) step through the sheets.

## The board and day breaks

The Board tab shows each sub-board as its shoot days, then the Boneyard.

- **Move strips** by dragging, or with Option+Up and Option+Down. Shift-click selects a
  run, Command-click adds one.
- **Day breaks**: Board > Insert Day Break Below (`⌘D`, or `B` on the board). To break a
  whole sub-board at once, pick what limits a day (pages, shoot time or screen time), type
  the limit and choose Break. A day closes as soon as it reaches the limit, so a long scene
  can run a day over.
- **Banners** (`⌘B`) mark a company move, a block or a note, with a color and a duration
  that counts toward the day's time.
- **Strip colors** and **Strip Layout** (Board menu) set what the strips show and how
  they look. The usual colors are white for interior day, yellow for exterior day, blue
  for interior night and green for exterior night.
- **Sort Strips** sorts the strips by up to four fields.

## The calendar and events

The Calendar tab shows a month (or every week in one list) of the picked sub-board. Pick a
date to make it a working day, a day off, a holiday or a company travel day. Holidays and
travel days take no shoot day number, so later days move to the next working date.

**Events** book elements on dates: travel, fittings, rehearsals, unavailable dates,
camera tests, post-production work and anything else. Travel, fittings and rehearsals
count as work in the day out of days. The others show on the calendar only.

## The day out of days, in plain words

Each row is an element, each column a date from its first day to its last.

- **Work** days are the days its scenes are scheduled, plus travel, fittings and
  rehearsals.
- **Holds**: with holds on for an element, every working day between its first and last
  work day that it doesn't work is held, and is paid. Days off stay off and count
  nowhere.
- **Drops**: with drops on, an element gets at most one drop, in its longest gap between
  two work days, when that gap is at least its minimum (counted in calendar days,
  weekends and holidays included). A tie goes to the earlier gap. The days inside the
  drop count nowhere. The last work day before it carries a `D` (`WD`) and the day it
  comes back a `P` (`PW`). Every other gap is held.
- **Holidays** (`/`) **and company travel days** (`M`) inside the work span count as
  worked only when holds are on and they are not inside the drop.
- **An event on a held day** shows the event's code with `H` and counts as worked. An
  event at the edge of a drop takes the `D` or `P`.
- **Total** is work plus hold.

New elements start with holds and drops off. Turn them on per element with the **H** and
**D** boxes on its row, and set its minimum drop gap on the Elements tab (or a category's,
for the elements added to it later). When a count surprises
you, click it: **Explain this count** lists every gap and what became of it, and why.

## Reports and exports

The Reports tab previews exactly the pages that print. Pick a report, set its dates, paper,
header and options, then **Print** or **Save as PDF**. Copy a report to keep two versions
of it with different options. Production Info sets what the full header prints.

**File > Export** writes a report as a spreadsheet (.xlsx) or a CSV: the day out of days, the
one-line schedule, the element list, the breakdowns and more. Totals in a workbook are
working formulas.

The Plan menu has the planning tools: script revisions and the drafts they recorded (each
with its color, date and changed scenes), the Day Timeline (call to wrap, sunrise and
sunset, and each person's own call and wrap), map places for locations, turnarounds and
rest, call sheets (printed, saved as a PDF, or as data), schedule locks and a suggested
strip order.

## Red flags and conflicts

A **red flag** says an element can't work on some dates (hard) or would rather not (soft):
a range of dates, or every date before or after one. Add them on the element (Elements
tab) or for several elements at once on the Red flags tab. A strip whose day breaks a flag
is marked on the board, and the Red flags tab lists every clash with its scenes.

A **conflict** is one element on the same date in two sub-boards of a board, or a travel
day, fitting or rehearsal on a day it works. The Conflicts tab lists them, with a count in
its badge.

## Merging and working together

- **Changes made outside the app.** When your schedule's file changes on disk (a sync
  folder, another computer, a colleague), nothing is written over it. The app asks:
  Load Theirs, Keep Mine, or Merge when you both changed it. Later puts the question off.
- **What Changed** (File menu) lists every change between this schedule and another file,
  or since you last saved. Revert to Backup's list has it too, for one backup.
- **Merge Changes From** brings what changed in another copy of the schedule into yours,
  against the version you both started from. Where you both changed the same thing, you
  pick yours or theirs. The result always hangs together.
- **With git**, `method-schedule merge-driver` does the same merge for every schedule file
  in a repository. Set it up once with `method-schedule git-setup` in the repository (or
  `method-schedule git-setup --global` for all of them), and a merge either goes through or
  marks the file for you to settle. The file stays a schedule either way: open it, check
  the listed conflicts, then `git add` it.

## The budget exchange file

**File > Export for Budget** writes one small file a budgeting app can read: shoot days by
unit, set, location and week, the date of every shoot day, and each element's work, hold,
travel, fitting, rehearsal and holiday counts, kept apart. It carries two hashes: one of
the whole schedule, and one of just the days it reports, so the budget can tell a change
in days from an edit that doesn't touch them. The command line writes the same file with
`method-schedule budget`. The format is docs/EXCHANGE.md, version 1.

## Keyboard shortcuts

Choose **Help > Keyboard Shortcuts** (`⌘/`) for every command and its keys, with a filter.
The menus show the same keys.

The Help menu also has **What's New** (this version's notes), **Report a Bug** (a new bug
report in your browser, with the version filled in: describe it with an invented schedule,
never a real one) and **Licenses and Notices**.

## Files and backups

- A schedule is one `.mtsc` file. Templates are `.mtst` files and never get written over by
  opening them.
- **Autosave** saves into the file a few seconds after you stop changing it, and when you
  switch away. A schedule never saved gets a recovery copy instead.
- **Recovery copies** cover a crash or a forced quit. The next start offers them under
  Recovered schedules: open one to get your work back, or discard it.
- **Backups**: before a save replaces a file, the version on disk is kept. The last five
  per file are kept. **File > Revert to Backup** brings one back as a single step you can
  undo.
- A file that is locked or read-only is never written over. The app says so and keeps a
  recovery copy; Save As puts your work somewhere else.
- **File > Open Recent** lists the last ten schedules.
