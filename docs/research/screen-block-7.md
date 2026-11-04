# Screen block 7 (2026-11-04, results, cut short)

The owner gave the block in chat and needed the computer back about twenty minutes in.
Everything ran in the background: no window brought to the front, no full-screen control
(it was asked for once, for a save panel, and not approved). A debug `Method Schedule.app`
built from main at 103b1b8 (`cargo tauri build --debug --bundles app`, Tauri 2.12.1) ran
on two copies of the sample production (`method-schedule sample --demo`). The reference
app ran on a copy of its ZTEST file. Both apps were quit without saving the test edits,
and the shared screen lock was held for the block and released after it.

## Method Schedule: passed

| Check | Result |
|---|---|
| D219 | With one schedule open, a second `.mtsc` sent through Launch Services (the open-document event a Finder double-click sends) opened in its own window; the first stayed as it was. |
| Tauri 2.12 | The app opens, every menu is there (Apple, Method Schedule, File, Edit, View, Board, Breakdown, Plan, Window, Help), and File > Save As opens the system's save sheet on the file's folder; Cancel closes it. What's New for 0.7.0 showed on the first start, as it should. |
| K184 | With Board Beside Board on (the main board, and Second Unit beside it), Find for SALTMARSH listed 3 matches, Return opened Scene 11's sheet, and Select on Board selected all three ("3 strips selected"), outlined in the side pane, the main pane scrolled to the first. |
| M192 | Day Timeline on a day with shoot times, the long day set to 1:00: "2 strips run past +1:00 on day 1: Sc 16, Sc 32. They would move to the start of day 2." Move to Day 2 moved them, the board followed, and the footer said "Moved 2 strips to the start of day 2. Undo puts them back." |
| D243 | Crew calls for one day in the real window: with a 07:00 call, Stunts added for the day at +1:00 shows a call of 08:00 in the Day Timeline's Crew table. |
| Red Flag Types rename | Travel renamed to Travel day, then one click on Done: the dialog closed and the new name was kept (reopened to check). |
| K185 (screen) | A named layout (Compact) saved from Strip Layout with the \| rule after Set and the _ rule under Scene: both draw on every strip of the board. |
| K187 (screen) | A banner set to Monospaced (`method-schedule banner FILE 56 --font mono`) draws in that face on the board. |

## Bug found

1. Red Flag Types printed the word "null" under the New type field whenever no standard
   type was missing: the empty slot went to `replaceChildren` as `null`, which the page
   writes as text. Fixed in `redflags.js` (an empty string instead).

## The reference app

- **E-L5, the open half**: yes, there is a fixed built-in set. Its Elements tab has a
  Built-in list beside the categories: D/N, Episode, I/E, Location, Script Day, Sequence,
  Set and Unit, each listing the values in use as elements (D/N had 8: Dawn, Day, Dusk,
  Evening, Morning, Night, Sunrise, Sunset). They are the breakdown sheet's own fields, not
  categories; ours keeps them as sheet fields (decision 42 stands).

## Not run

Saving the board as a PDF (K185 and K187 on paper): the save panel would not take a folder
in the background, and full-screen control was not approved. Next time keep the test files
in `~/Documents` so the panel opens on an ordinary folder. Also not reached: the Layouts
rename (the sample had no named layout until Compact was saved), crash recovery
(`scripts/crash-check.sh`), the update and both-chips tests, E-S34, and the two report
questions (RPT-03, RPT-10).
