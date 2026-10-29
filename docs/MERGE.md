# Diff, merge and "explain this count"

Schedule files carry a stable id on everything (docs/FORMAT.md): categories, elements,
sheets, boards, sub-boards and strips. Ids survive edits and moves, so two versions of a
schedule can be compared and merged by identity, never by position. The engine does it
(`method_schedule_core::diff`, `method_schedule_core::merge`, `method_schedule_core::explain`); the command line and the
MCP server use the same code. The design is Method Budget's (its docs/MERGE.md and
decisions 43 and 44), same owner and license. Decisions are in docs/decisions.md, entry 25 (the app's screens: entry 26).

## What changed (diff)

`diff(a, b)` compares an older schedule `a` with a newer one `b` and lists every change,
one plain sentence each, in a fixed order:

1. the title;
2. categories, red flag types, elements, red flags, then sheets: each in `b`'s order
   (added and changed), then what went;
3. board by board in `b`'s order: the board itself, its sub-boards (name, start), each
   sub-board's calendar, then its strips in board order (each sub-board, then the
   Boneyard), then strips and sub-boards that went; boards that went last.

| What | Matched by | Reported |
|---|---|---|
| Title | | `Title "Harbor" -> "Harbor Light"` |
| Categories | id | added, removed, changed field by field (name, automatic board IDs, left off the board, default holds, drops and minimum drop gap), moved in the list |
| Red flag types | id | added, removed, renamed, moved in the list |
| Elements | id | added, removed, changed field by field (category, name, board ID, lock, holds, drops, minimum drop gap, notes, links as `+DOG -CAT`) |
| Red flags | id | added, removed, changed field by field (element, type, dates, note, hard): `Red flag "Unavailable" (on 2027-03-02) on "ADA" (Cast): dates on 2027-03-02 -> not after 2027-03-20, hard off -> on` |
| Sheets | id | added, removed, changed field by field (every header field, pages as `1 3/8`, times as `h:mm`, elements as `+KIT -TOVE`) |
| Boards and sub-boards | id | added, removed, renamed, moved in their list; a board's strip layout (the whole layout, as `Scene, Set (widest), Cast IDs`: a field shown with its width when that isn't the field's default); a sub-board's start date |
| Calendars | date | weekly days off as a whole; each date's mark (day off, holiday, company travel day, marked back to working) added, removed or changed |
| Events | value | added or removed (events have no ids yet, so an edited event reads as one removed and one added) |
| Strips | id | added, removed, banners changed (text, time), and **moved, strip by strip** |

A strip is *moved* when it went to another sub-board or the Boneyard, or when it changed
place in its own list and falls outside a longest run of strips that kept their order (a
longest increasing subsequence). So dragging one scene to the top reports that scene, not
every strip it passed. Positions count from 1 in each list and give the shoot day:

```
Title "ZTEST Lantern Creek" -> "ZTEST Lantern Creek II"
Category "Props" added
Element "ADA" (Cast): board ID none -> "1", holds off -> on
Scene 1: set "CABIN" -> "LOG CABIN", pages 1/8 -> 1 3/8, elements +CARL
Sub-board "Main" (board "Board 1"): start 2027-03-01 -> 2027-03-08
"Main" calendar (board "Board 1"): holiday 2027-03-03 added
"Main" calendar (board "Board 1"): 2027-03-03 holiday -> company travel day
"Main" calendar (board "Board 1"): event fitting "Wardrobe" on 2027-03-02 (BEN) added
Board "Board 1": scene 3 moved on "Main" from position 5, day 3 to position 1, day 1
Board "Board 1": scene 2 moved from "Main" (position 3, day 2) to the Boneyard (position 1)
Board "Board 1": banner "Company move" added on "Main" at position 1, day 1
Board "Board 1": day break removed from "Main" (position 4, closing day 2)
```

A sheet strip that went with its sheet is not listed again. Not listed: `next_id` and the
order of the element and sheet lists, which is creation order, not content.

Each change also carries its kind (`added`, `removed`, `changed`, `moved`), what it is
about (title, category, element, sheet, board, sub-board, calendar or strip, with ids) and
each field's before and after, for programs (the MCP `diff` tool returns them).

Properties (`method-schedule-core/src/diff.rs`): a schedule compared with itself has no changes;
random edits through the engine always show up.

## Three-way merge

`merge(base, ours, theirs)` takes a common ancestor and two schedules made from it and
returns one schedule with both sets of changes, the conflicts, and anything the merge had
to put right.

- **A change on one side only is taken. The same change on both sides is taken once.**
- **Different things, or different fields of one thing, merge on their own**: ours
  changing a sheet's set and theirs its synopsis keeps both; holds on one side and drops on
  the other keeps both. A sheet's elements and an element's links merge id by id (one side
  tagging KIT and the other DOG keeps both; a tag one side took off goes). Calendar dates
  merge date by date; events by value. Red flag types merge like categories (a name
  field, the list order like the categories', a new red flag using one counts as changing
  it); red flags merge id by id inside their element, field by field (type, dates, note,
  hard), so one side's note and the other's dates both stay. The planning data merges
  list by list (decision 91): a map location by name, a call time by its day break, a
  turnaround by category and a schedule lock by name, each entry one value; pinned
  strips and omitted scenes as sets; the long day limit as a field. So one side's call
  time and the other's map location both stay, and two different call times for one day
  conflict under `plan:call_time:<day break id>`.
- **Conflicts**, each settled for **ours** by default and listed with ours, theirs and the
  base value:
  - the same field changed to different values on both sides;
  - something deleted on one side and changed on the other. "Changed" includes being used
    anew: an element tagged on a sheet, put on an event or linked; a category given new
    elements. A banner or day break counts as changed when it was edited or moved to
    another list;
  - both sides reordering the same list differently: a sub-board's strips, the Boneyard,
    the categories, a board's sub-boards;
  - one strip moved to different lists on both sides (its place);
  - a board's strip layout changed differently on both sides. The layout merges as one
    value (its fields, order and widths together), so one side's new field and the
    other's new width conflict rather than mix (decision 39).

  ```
  Scene 1 set: ours "LOG CABIN", theirs "BARN", was "CABIN"
  Element "CARL" (Cast): ours deleted it, theirs changed it
  Board "Board 1", "Main" strip order: ours scene 3, ..., theirs scene 2, ..., was ...
  Board "Board 1": scene 2 place: ours the Boneyard, theirs "Second unit", was "Main"
  ```
- **Keeping something one side deleted** keeps it as the side that changed it has it,
  with what the deleting side took away with it: an element keeps its tags on sheets both
  the base and the keeping side had, and its links; a sub-board keeps the strips the
  deleting side moved to the Boneyard or dropped with it. Deleting it wins over the other
  side's new uses: tags on a deleted element go.
- **Strip order** follows Method Budget's rule for rows. Only strips in all three versions
  of a list count as its order. If one side reordered them, that side's order wins; if
  neither did, ours. Strips the other side added, or moved in from another list, go right
  after the strip they follow there. Both sides reordering differently is a conflict
  (ours' order by default), never an interleaving. Where a strip lives (which sub-board, or
  the Boneyard) merges like a field. A sheet strip goes with its sheet.
- **New ids on both sides.** Each copy numbers new things from its own `next_id`, so two
  copies adding things usually reuse ids. Categories added on both sides with one name
  (ignoring case) are one category; elements added on both sides with one name in one
  category are one element (their fields merge, a difference is a conflict), and red flag
  types added on both sides with one name are one type. Anything else
  added on both sides with one id is one thing only when it is identical (a strip on the
  same board and list, a sub-board on the same board); otherwise theirs, and everything
  hanging from it, gets new ids above every id in the three schedules. The merge reports
  how many.
- **The result always passes `Schedule::check`.** Where the merged parts don't fit
  together, the merge falls back to ours for the part at fault and says so. A merge settled
  for theirs (`--theirs`, `merge_with(.., Side::Theirs, ..)`) falls back to theirs
  instead, so it gives what the merge the other way round gives settled for ours
  (decisions.md, entry 174). Below, "ours" is the side the merge settles for:
  - an element one side renamed to the name of an element the other side added: one
    element, the renamed one, which takes the added one's tags and links whichever side
    the merge settles for (entry 236);
  - two categories or two elements in a category with one name: the later one gets ours'
    name back if the merge changed it, else the first does; two that ours never had apart
    join (an element is merged into the first like the Elements tab's Merge);
  - two elements in a category with one board ID: ours' board ID back, else none;
  - an element whose category is gone: ours' category, else theirs', else the base's,
    else it is left out;
  - tags, links and event entries for an element that is gone are left off;
  - two red flag types with one name: like categories, ours' name back, else the later
    one joins the first; a red flag type one side deleted that a kept red flag still uses
    stays; a red flag on two elements stays where ours has it;
  - a board left without sub-boards or every board deleted between the two sides: ours'
    boards;
  - a board missing a sheet (a board one side added, a sheet the other added): the sheet
    goes to the end of that board's Boneyard; a sheet both on a sub-board and in the
    Boneyard keeps its place on the sub-board, twice in the Boneyard ours' strip;
  - a property value one side set under a property the other side renamed follows the
    rename; a value set under a property that is gone is left out, with a line;
  - a part of a scene spanning days that one side deleted goes, unless the other side
    changed it; each side deleting a different part keeps the first, so the sheet keeps
    a strip;
  - as a last resort, a merged schedule that still doesn't hang together is ours whole,
    with the reason. The property tests never reach it.

Conflict keys are stable for the same three schedules (`sheet:4:set`,
`board:2:strips:3`, `element:12`), so a caller can show the conflicts, take a pick per key
and merge again with `merge_with(base, ours, theirs, default, picks)`.

Properties (`method-schedule-core/src/merge.rs`, random schedules and random edits through the
engine, 300 cases each in the suite, more in the weekly fuzz job): `merge(base, x, base) == x`,
`merge(base, base, x) == x` and `merge(base, x, x) == x`, each with no conflicts and no
fallbacks; any two edit sets merge, settled either way, into a schedule that passes
`Schedule::check`; the same merge gives the same conflicts; and `merge(base, a, b)`
settled for ours is `merge(base, b, a)` settled for theirs, with the same conflict keys and
as many fallbacks (decision 174). That last one holds up to what neither side decides:
things both sides add or put at one place go ours' first, so only their order may differ,
and when both sides add things the same things may get other ids. `diff(a, a)` is empty.

## Explain this count

`explain(schedule, board, sub_board, element)` says why one element's day out of days
totals on one sub-board are what they are, by the rules in decisions 10 to 12. It is
worked out with the engine's own pieces (`dood::dood` and `dood::gaps`, which the day out
of days uses to pick the drop), and a property test holds every code and count to the day
out of days on random schedules.

It returns the totals (work, hold, total), what work is made of (shoot days worked, event
days, holidays and company travel days that counted), the rules, the drop, every gap and
every date of the row, each with a sentence:

```
"ADA" (Cast) on "Main" (board "Board 1"): work 5 + hold 8 = total 13, start 2027-03-01, finish 2027-03-30.
Work 5: 4 shoot days worked, 1 holiday counted as worked.
Hold 8: shoot days between the first and last work day that are not work, event or drop days.
Rules: holds on, drops on, minimum drop gap 7 days.
The drop: 13 days between 2027-03-16 and 2027-03-30, the longest gap of at least 7 days; the days inside count nowhere.

Gaps
  6 days between 2027-03-01 and 2027-03-08: held: shorter than the minimum drop gap of 7 days
  7 days between 2027-03-08 and 2027-03-16: held: a longer gap (13 days between 2027-03-16 and 2027-03-30) took the one drop
  13 days between 2027-03-16 and 2027-03-30: dropped: the longest gap of at least 7 days

Day by day
  SW   Mon 2027-03-01, shoot day 1: works (scene 1); the first work day (S)
  H    Tue 2027-03-02, shoot day 2: held, a shoot day between work days with holds on
  /    Wed 2027-03-03: a holiday inside the work span, holds on and outside any drop, so it counts as worked
  .    Sat 2027-03-06: a day off, counts nowhere
  .    Sat 2027-03-20: a day off inside the drop between 2027-03-16 and 2027-03-30, counts nowhere
  PWF  Tue 2027-03-30, shoot day 21: works (scene 21); the pick-up after the drop (P), the last work day (F)
```

Each date has a reason: work (with its sheets), a counted event (travel, fitting,
rehearsal, also outside the work span), a counted event on a day that would be held
(`FTH`, worked not held), a hold, a holiday or company travel day that counted, holds off,
inside the drop, a day off, or outside the work span. Other events on the date are named
with "counts nowhere". Each gap says its length in calendar days (weekends and holidays
included, event days inside the span splitting it) against the minimum, and whether it
was dropped, held because it is too short, held because a longer (or an earlier, equally
long) gap took the one drop, held because drops are off, or counted nowhere because holds
are off.

## Command line

```
method-schedule diff OLD NEW
method-schedule merge BASE OURS THEIRS [--out FILE] [--theirs]
method-schedule merge-driver BASE OURS THEIRS [PATH] [--theirs]
method-schedule explain FILE ELEMENT [--category CAT]
```

- `method-schedule diff` prints the changes grouped under headings (Schedule, Categories, Elements,
  Sheets, Boards, Calendars, Strips), or `No changes.`. Exit 0.
- `method-schedule merge` writes the merged schedule to `--out` (atomically; never over one of the
  three inputs) and prints what it did, or, without `--out`, prints the merged file on
  stdout and the report on stderr. The report lists ids given to theirs, every fallback and
  every conflict with the side taken. Exit 0 with no conflicts; **exit 1 when conflicts
  were settled** (the merged file is written either way, so it can be checked); exit 1 too
  for an error, with nothing written. `--theirs` settles conflicts for theirs instead.
- `method-schedule merge-driver` is the same merge for git: it writes the result over `OURS`.
- Every conflict either one settles is kept in the merged file (`plan.conflicts`,
  docs/FORMAT.md) until someone marks it done in the app's File > Merge Conflicts to Review,
  so a merge git ran for you leaves the list with the file (decisions.md, entry 238). The
  report ends with a line saying so.
- `method-schedule explain` explains an element on the first board's first sub-board (it needs a
  start date). Names ignore case; a name in more than one category needs `--category`.

## With git

`method-schedule git-setup` does what follows in the repository it is run in (add
`--global` for every repository, which writes git's own attributes file,
`core.attributesfile` or its default; add `--theirs` to settle conflicts for theirs). It
adds the attribute line once and says whether it was there already. By hand:

Tell git which files use the driver, in the repository's `.gitattributes`:

```
*.mtsc merge=method-schedule
```

and what the driver is, once per repository (or with `--global`):

```
git config merge.method-schedule.name "Method Schedule merge"
git config merge.method-schedule.driver "method-schedule merge-driver %O %A %B %P"
```

`%O` is the common base, `%A` ours (which the result replaces), `%B` theirs and `%P` the
file's path, for messages. A clean merge is committed as usual. When conflicts remain the
driver still writes a valid schedule, every conflict settled for ours, lists them on
stderr and exits 1, so git marks the file conflicted (`UU`) without writing conflict
markers into it, which would make it unreadable. Check the listed conflicts (in the app,
or with `method-schedule diff`), fix what needs fixing, then `git add` the file. When both branches
added the same file there is no base (git passes an empty one): the driver says so, exits
1 and leaves ours as it was. Add `--theirs` to the driver line to settle conflicts for
theirs instead. Schedule files use the `.mtsc` extension (decisions.md,
entry 27).

`crates/method-schedule-cli/tests/merge_driver.rs` runs a real `git merge` in a temporary folder
with this setup: two branches that changed different fields merge cleanly, two that
changed one field stop with `UU` and a valid schedule settled for ours.

## For agents (MCP)

`method-schedule mcp` has two read tools for this (docs/MCP.md): `diff` `{path, other}` and
`explain` `{path, element | name, category?, board?, sub_board?}`. Merging is a command
line and git job for now; there is no merge tool.

## In the app

File > What Changed… and What Changed Since Saved list `diff` from another file or the
schedule's own file on disk to the schedule in the window. File > Merge Changes From…
merges another copy against a base found in the app's rolling backups (the newest backup
made before the copy was last changed, else the file on disk, saying why), with Keep
yours or Keep theirs per conflict, as one undo step. When the schedule's own file was
changed outside the app while the window has unsaved changes, the same dialog merges the
file on disk in, against the version the window last read from or saved into it
(decisions.md, entry 180). On the Day out of days tab a row's
Work, Hold or Total opens `explain` in a panel beside the grid; a date there scrolls the
grid to it. docs/APP.md has the details; decisions.md, entry 26, the choices.
