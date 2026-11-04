# Engine checks: questions to settle in the real MMS app

Status: October 2026, research phase. Nothing here has been run in MMS yet. Every "proposed
rule" is our best reading of the public manual and older material (behavior notes 01 to 13)
and stands as the engine default until the real app says otherwise. When a check is run,
record the result in the "MMS result" column, then add a decision entry and a fixture.

Clean-room: behavior in our own words, no copied text, the sources stay outside the repo.
All test data is invented. Tests use throwaway schedules named `ZTEST ...`, made fresh in
MMS, never the owner's real files. Exports go to the gitignored `/imports/`.

## Standard setup (used by most checks)

- New schedule `ZTEST Lantern Creek`. One board, sub-board "Main".
- Start date Monday 2027-03-01. Working week Monday to Friday. No other off days unless the
  check says so.
- Category Cast with invented elements ADA (board ID 1), BRUNO (2), CLEO (3).
- Sheets 1 to 25, one per scene, each 1 page, shoot time 1:00, no other elements unless the
  check says so.
- One strip per shoot day: a day break after every strip, so Day N is the Nth weekday:
  Day 1 Mon Mar 1, Day 5 Fri Mar 5, Day 6 Mon Mar 8, Day 10 Fri Mar 12, Day 11 Mon Mar 15,
  Day 15 Fri Mar 19, Day 16 Mon Mar 22, Day 21 Mon Mar 29.
- "Put ADA on Day N" means add ADA to the sheet on that day.
- Read results on the DOOD screen (category Cast), and export the DOOD summary to Excel.

## Results from the real app

Run 1, 2026-10-10 (behavior note 14). These replace the proposed rules below.

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-D1 | Gap = calendar days strictly between two work days; 13 days drops at minimum 13, holds at 14 | As proposed |
| E-D2 | One drop per element, but in the **longest** qualifying gap, ties to the earlier one | Changed: longest, not first (D10) |
| E-D3 | A holiday inside the gap counts toward the minimum | As proposed |
| E-D5 | H on working days strictly between; weekends uncounted | As proposed |
| E-D7 (part) | A holiday inside the span with holds on counts as worked; inside a drop or outside the span it counts nowhere | As proposed; summary export columns still to read |
| E-D13 | Holds off: holiday not counted | As proposed |
| E-B1 (part) | A strip exactly at the limit fits the day | Inclusive, as proposed |
| New | Imported cast get no board ID | Changed: import leaves board IDs empty (D10) |
| New | New element: holds off, drops off, minimum gap 1 when drops are turned on | Defaults set to match (D10) |
| E-I4 (new) | `.fdx` `Length="1"` arrived as 1/8 | Open: try `8/8` and `1 0/8` |

Run 2, 2026-10-10 (behavior note 15).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-D8 | Fitting on a hold day shows `FTH` and counts as worked, not held | As proposed, plus the joined code (D11) |
| E-D10 | Event before the start: columns extend back, event counts as worked, S unchanged | As proposed |
| E-D11 | Travel inside a drop gap splits it; the drop goes if no part meets the minimum | As proposed |
| E-D17 | Company travel day: no day number, days reflow; `M` counts as worked inside the held span | As proposed |
| E-D7 | Summary export columns: name, Work, Hold, Holiday, Start, Finish, Total; Total = Work + Hold | Matched (D11) |
| E-D14 (part) | With no board IDs, rows sort by name | Sort by board ID, then name |
| E-D20 (new) | The summary's Holiday column stayed empty although holidays counted in Work | Open |

Run 3, 2026-10-10 (behavior note 16).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-I4 | `8/8` and `1 0/8` read as 1 page, `10/8` as 1 2/8, a bare `1` as 1/8 | `Eighths::from_script` (D12) |
| E-B1 | A day closes once its total reaches or passes the limit; it can run over | Changed (D12) |
| E-B2 | A strip over the limit joins the open day and closes it; at a day's start it fills the day alone | Changed (D12) |
| E-B6 | A break closes the last day, even a zero-length one | As proposed |

Screen block 1, part A, 2026-10-11 (behavior note 17; runbook items A1 to A15).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-D14 | Rows by board ID in natural order: 1, 1a, 2, 7, 07, 9, 10; no ID last; ties by name (A1, A2) | Changed: natural order (decision 40) |
| E-L4 | Names unique ignoring case (A3); `07` and `7` both allowed, kept as typed (A2) | As proposed |
| New (A14) | Next automatic board ID is one past the highest, not the lowest free | Changed (decision 41) |
| E-L5 (part) | The cast category can be renamed (A10) | Changed: renaming allowed, still can't be moved or deleted (decision 42) |
| E-D6 | Event on a work day: the event's code with the work marks (`FTP`), a warning, counted once, a conflict (A4, A5) | Changed (decision 43); the whole work code follows the event's since block 2 (`TRSW`, decision 47) |
| E-D23 (part) | A holiday on a Saturday inside a held span shows `/` and counts as worked (A6) | As proposed |
| E-C9 | A Saturday marked working becomes a shoot day and later days move (A7) | As proposed |
| E-E3 (events) | Unavailable and camera test events neither show nor count (A8) | As proposed |
| E-D20 | The summary's Holiday column stays empty (A9) | Still not modeled |
| E-L6 | Merge offers only the selected elements as survivor, so one category; the survivor keeps its ID, gains the scenes and the others' events (A11) | As ours: merge_elements refuses mixed categories (decision 45) |
| E-L7 | Moving into a category where the board ID is taken is refused with a message (A12) | Changed: refused (decision 45) |
| E-L9, E-D19 | Category settings reach new elements only; no apply to existing in the settings (A10, A14) | We keep our explicit Apply to Existing Elements |
| E-B3 | A banner's time counts toward the day; 3:00, 1:30 and 3:00 close a 7:30 day (A15) | As proposed |
| E-B12 (part) | New Banner goes above the selected strip | Changed: ⌘B inserts above (decision 44) |
| E-L8 (part) | Renumber goes in list order and leaves a locked ID in place (A13) | As proposed; whether a locked number is skipped is open |

Screen block 2, 2026-10-11 (behavior note 18).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-S18 | The first selected strip survives (not the topmost); scenes join in selection order, the survivor's first (`3, 2`); the merge applies on every board | Changed (decision 46) |
| E-D6 | Travel on a start day reads `TRSW`: the event's code, then the whole work code | Changed (decision 47): `FTW`, `FTPW`, `TRSW`, `FTWF` |
| E-S19 | Paste (from the strip's context menu) puts the strips below the strip pasted on; a moved (cut) strip is selected after, a pasted copy is not; Paste is greyed until something is cut or copied | Below the bottommost selected, as proposed; ours selects a pasted copy too (decision 52) |
| E-S20 | Cut leaves the strip where it is, outlined in red, until the paste moves it; the strip count stays the same | As proposed (decision 38) |
| E-S21 (part) | Copy and paste makes a new sheet with the same scene number (`3`, not `3A`) | As proposed (decision 38); pasting on another board not tried |
| E-S22 (part) | Named layouts in a list, each with a type (strips, day breaks, shoot day); the default strip layout has two lines | Named layouts of the three types, picked per board and by the one-line report; two-line strips; a new board's default stays one line (decision 78) |
| New | The Edit menu's Cut and Paste did nothing to a selected strip; strips are cut, copied and pasted from their context menu | Ours acts from the Edit menu too, a deliberate difference (decision 52) |

Screen block 3d, 2026-10-12 (behavior note 22).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-S17 | Duplicate Sheet puts the copy right after the original, same day, same scene number, selected | As proposed |
| E-S12 (part) | Boards: a list with an edit button each, Add Board, Reorder; edit renames, duplicates, deletes | As proposed |
| E-S13 | Delete Board is off while it is the only board | As proposed |
| E-S25 | A strip color and a text color per I/E and time of day pair | Changed: a text color per pair (decision 108) |
| New | Calendar: weeks in one scrolling list, holiday star, travel plane, an Actions menu, View switches | Added beside the month (decision 111) |
| New | Events: a color from ten, any number of elements, post-production kinds | Added (decision 109) |
| New | Split views: breakdown beside images | Added (decision 110) |

Parity marathon, 2026-10-12 (behavior notes 24 and 25).

| ID | Result in the real app | Our rule now |
|---|---|---|
| E-L2 | Links chain: actor to prop to a second prop, adding the actor added both, one undo removed all three; links are one way; without retroactive nothing changes on sheets that already hold the element | Changed: every level, a cycle guard (decision 116) |
| E-L8 | With one element locked at 2, renumbering from 1 gave 1, (2), 3, 4, 5, 6: a locked number is skipped | As ours |
| E-L11 | A new background element counted as 1 with no quantity set; its sheet's use in daily quantity is on | As ours (decision 63) |
| E-L12 | A sheet's set typed `room 2` used the existing `ROOM 2`: values match ignoring case | Changed: the sheet takes the existing spelling, and the field lists the values as typed (decision 155) |
| E-D20 | A holiday inside held spans counts in Work; the summary's Holiday column stays empty | Changed: ours has no holiday count in the summary; the exchange keeps its own (decision 117) |
| E-E15 | A travel red flag left the actor's work and hold totals as they were; a flag mark shows by the name | As ours (decision 113) |
| E-E16 | Three dates (two following, one apart) made two flags, one per run of following dates | As ours (decision 114) |
| E-B14 | Remove all day breaks does not ask; the shoot day headers go with them; Undo restores them | As ours (decisions 56 and 125) |
| New | Header pseudo-categories cover every one-value header field, unused pick list values included | Changed: eight fields (decision 118) |
| New | The cast category comes with contact and deal properties; a property is one line or several | Changed: a starter set and the two types (decision 119) |
| New | Category settings are defaults for new elements; exclude from board is the element's | Changed (decision 120) |
| New | The DOOD View menu hides or shows Saturdays and Sundays, a switch for each | Added, per viewer (decision 121); a switch each since decision 158 |
| New | Board sort: at most four rules, no direction choice | Changed: four, directions kept (decision 122) |
| New | A new sub-board's start date is today | Changed (decision 123) |
| BRD-01 | A new board's one sub-board is dated today, every sheet in its Boneyard in sheet order | Changed: ours dated it nothing (decision 153); its name stays Main (the reference app's is Default) |
| New | Conflicts list header values (I/E, time of day) too | Ours keeps elements only, on purpose (decision 124) |
| New | Delete Sheet deletes without asking, a notice, Undo | Ours keeps asking, on purpose (decision 125) |
| New | New Banner edits in place on the board | Ours keeps its dialog (decision 126) |

## Queue for the next real-app runs (2026-10-11)

From building the Elements tab, DOOD grid and calendar (decisions 19 to 21), plus run 4's
list. Our current rule stands until checked. The runbook for the next block of screen
time is `screen-block-1.md`.

| ID | Question | Our rule today |
|---|---|---|
| E-D6 | An event on a day the element works: which code, counted once? | Checked (notes 17 and 18): the event's code, then the whole work code (`FTW`, `FTPW`, `TRSW`), flagged, counted once as worked, and a conflict (decisions 43 and 47). A finish day (`FTWF`) by the same rule, not seen |
| E-D14 | DOOD row order once elements have board IDs | Checked (note 17, A1 and A2): natural board ID order (1, 1a, 2, 7, 07, 9, 10), no ID last, ties by name (decision 40) |
| E-D20 | What fills the summary's Holiday column? | Checked (note 17, A9; note 25): it stays empty, the holiday counted in Work; ours has no holiday count in the summary (decision 117) |
| E-B3 | Auto breaks by time with banners | Checked (note 17, A15): banners count, a day closes on reaching the limit |
| E-L4 | Names unique ignoring case? Board IDs: `1a` vs `1A`, `07` vs `7` | Checked (note 17, A2 and A3): names ignore case; board IDs exact text, `07` and `7` both allowed. `1a` against `1A` not tried |
| E-L5 | Is Cast a fixed built-in? Is there a fixed built-in set? | Checked (note 17, A10; screen block 7): the cast category can be renamed; it stays built in otherwise (first, not deleted) (decision 42). The fixed built-in set is the sheet's own fields, listed beside the categories as Built-in: D/N, Episode, I/E, Location, Script Day, Sequence, Set and Unit, each with its values in use as elements. Ours keeps them as sheet fields |
| E-L6 | Merge: same category only? Survivor's links and notes? | Checked in part (note 17, A11): same category only (the survivor is picked from the selection); survivor keeps its ID, gains the scenes and events. Ours refuses mixed categories too (decision 45). Links and notes not seen |
| E-L7 | Move to a category where the board ID is taken; does an auto-numbering category number it? | Checked (note 17, A12; note 27): refused with a message when the ID is taken, as ours (decision 45); an element with no ID moved into a category that numbers automatically keeps no ID, ours now the same (decision 140) |
| E-L8 | Renumber: elements with no ID? Do locked `04` block 4? | Checked (note 17, A13; note 25): list order, locked IDs stay, and a locked number is skipped, as ours |
| E-L9 | Apply to existing: does it include auto board ID? | Checked (note 17, A10 and A14): the real app has no apply to existing; settings reach new elements only. We keep ours (rules only) |
| E-D21 | DOOD grid range when an event comes before the first work day | Answered in run 2: the grid extends back to cover the event (note 15) |
| E-D22 | A drop that starts or ends on an event day: D or P shown? | Checked (note 27): the event acts as a work day for the drop; its code keeps the H, then D or P (`TRHD`, `TRHP`), and the work day beside it loses the letter; worked. Ours now the same (decision 139; `dood::tests::travel_beside_a_drop_moves_its_drop_or_pickup_as_the_reference_app`) |
| E-D23 | Holiday or company travel day on a weekend inside a held span | Checked for a holiday (note 17, A6): counts as worked. Company travel day not tried |
| E-C9 | How a weekend marked back to working shows | Checked (note 17, A7): a shoot day; later days move along |
| E-E3 | Unavailable and camera test events: kinds of their own? | Checked (note 17, A8; note 27, EVT-03): kinds of their own in the Add Event list, and they neither show nor count. The list ends with the post kinds, Color Correction last (decision 163); only travel, fitting and rehearsal count |
| E-C10 | Month view week start | Answered from run 1 screens: Sunday first |
| E-S5 | Can day breaks and banners be dragged into the Boneyard (inventory section 6 says breaks can be parked there)? | Day breaks park there (no day, no date, counted nowhere); banners are refused (decision 57, which replaced decision 29's refusal) |
| E-S6 | Dragging a selection that isn't contiguous: do the strips gather at the drop, in board order? | They gather at the drop in board order (each sub-board, then the Boneyard) |
| E-S7 | Shift-click and Command-click on strips: a range and a toggle? Do the arrow keys extend a selection? | Shift-click a range, Command-click (Ctrl-click) a toggle, Shift+Up and Down extend |
| E-S8 | Sort: where do sheets with an empty sort field go, ascending and descending? Is a tie kept in board order? | Empty last both ways; ties keep their order (decision 32) |
| E-S9 | Sort I/E and time of day: alphabetical, or a fixed order (INT before EXT, DAY before NIGHT)? | Alphabetical, ignoring case |
| E-S10 | Sort the Boneyard: offered? | Offered; it orders the scenes, and a parked day break goes as sorting removes day breaks (decision 57) |
| E-S11 | Sort scope: the selected sub-board only, or the whole board? | The list holding the selected strip |
| E-B11 | Banner text: required? Longest? A banner with only a duration? | Text required, 200 characters at most; duration optional (`h:mm`), 24:00 at most since ours took `900:45` (decision 50) |
| E-B12 | Insert banner with several strips selected: above the first, below the last? | Checked (notes 17 and 27): New Banner goes above the selected strip, so ⌘B is Insert Banner Above (decision 44); with several selected, above the bottommost. Changed: Above goes above the bottommost selected strip, Below below it (decision 143) |
| E-B14 | Auto day breaks by screen time: same rule as pages and shoot time (close on reaching the limit)? Do banners count? Does Remove All ask first? | Checked in part (note 24): remove-all doesn't ask and Undo brings the breaks and shoot day headers back, as ours (decisions 56 and 125). Screen time rule and banners: same rule, no screen time from banners, not seen |
| E-B15 | Banner styling: which colors (a palette or any), what "thin" and "standard" heights are, font and alignment options? | Nine fixed colors, one or two lines, bold (decision 65) |
| E-B16 | Day break extras: sunrise the earliest and sunset the latest of the day? Screen time the sum? Custom text the same on every break (a layout field) or per break? | Earliest, latest, the sum, one text per board (decision 67). Ours also takes the worked-out sun of a sheet whose location has degrees when none is typed (note 27, decision 167) |
| E-B17 | A scene spanning days (multiple day breaks): one sheet in parts? Whose day holds the pages and time? Does the scene's cast work every day? | One sheet, a strip per part; pages and times on the first part; its elements work every part's day (decision 76) |
| E-B13 | Delete key on banners and day breaks: does it ask? Day breaks too? | Banners ask; day breaks alone don't (Undo brings them back) |
| E-S1 (part) | Board names unique? Sub-board names unique per board or schedule-wide? | Boards unique ignoring case; sub-boards unique within their board (a duplicated board keeps its names) |
| E-S12 | New board: everything in the Boneyard of one sub-board? Its name? Duplicate's default name? | Checked (note 22): the board picker lists the boards, each with an edit button, then Add Board and Reorder; editing a board renames it and offers Duplicate Board and Delete Board, as ours. Ours: one sub-board "Main", every scene in the Boneyard; we suggest "<name> copy". Note 27: a new board has one empty sub-board (Default) dated today and every sheet in its Boneyard in sheet order; ours now dates it today too (decision 153). The copy's default name not read |
| E-S13 | Can the last board, or a board's last sub-board, be deleted? | Checked (note 22): Delete Board is off while it is the only board, as ours. No for a board's last sub-board too (ours; not tried there) |
| E-E5 | Conflicts: do held days count, or only work days (and do events, E-E2)? | Checked in part (note 17, A4): an event on a work day on the same sub-board is a conflict (decision 43). Held days: work days only, still to check; event clashes across sub-boards stay a separate informational list |
| E-E6 | Conflicts: strips below the last day break, a sub-board with no start date, a category excluded from the board | None of them conflict (E-D15, E-E3) |
| E-E7 | Find and replace: a rename that gives two elements of one category the same name | Refused, nothing changed (no merge) |
| E-E8 | Red flags: what triggers a clash? Work days only, or held days, holidays counted as worked and events (travel, fitting, rehearsal) on a flagged date too? Strips below the last day break, a sub-board with no start date? | Work days and counted events only; never holds, holidays, other events or unscheduled strips (decision 33) |
| E-E9 | How a red flag clash shows: on the board strip (icon, its color by type, the "plus" for several flags a day), on the calendar (the scheduled-only view and the list view of every flagged date), the DOOD cell and the breakdown sheet | Not drawn in the app yet; the engine lists clashes by date with the flag, the shoot day, the sheets and the events (decision 33) |
| E-E10 | Hard against soft: does a red flag (or its type) carry any severity, and does any flag ever block a move? | We store `hard` on each flag; both clash alike, warnings only, nothing blocked |
| E-E11 | Open-ended flags: can a flag run "from a date on" or "until a date" without an end? | Allowed: `not_before` and `not_after` beside a date range |
| E-E12 | Red flags on an element of a category excluded from the board: do they clash? | Yes: a flag is set on purpose (unlike conflicts, E-E3) |
| E-E13 | A conflict on a flagged element: in the conflict list too, or only the red flag report (note 03)? | Both: conflicts don't look at red flags |
| E-E14 | Red flag types: the built-in DOOD types (can't be deleted), colors, reorder; deleting a type still in use | Checked (note 23): red flag types and event types are one list with built-in entries (travel, fitting and rehearsal marked as counting, unavailable, camera test, the post-production kinds). Ours now starts new schedules and templates with a standard type per event kind but other, renamed and recolored but never deleted, older files keeping their own list until Add the Standard Types; a type and its event kind stay apart, so a flag never counts (decision 113). Colors and reorder as before; a type in use can't be deleted. Whether a flag of a counting type counts in the reference app's day out of days is not read: ours says no |
| E-E15 | A red flag of a counting standard type (travel, fitting, rehearsal): does it put its code on the day out of days and count as worked, or only warn? | Checked (note 25): only warns, the totals unchanged, a mark by the name, as ours (decision 113) |
| E-E16 | Red flags tab, several dates picked: one flag per date, or one per run of following dates? Several elements: one flag each? | Checked (note 25): one flag per run of following dates, as ours; several elements not seen (ours: one each, decision 114) |
| E-S3 (part) | Merge strips: which sheet's header wins (the first selected, the top one on the board, the one right-clicked)? Are scenes joined with commas, eighths, shoot time and screen time summed, synopses and comments joined, elements unioned? What happens to a one-value field (set, I/E, time of day, location, sunrise) that differs: kept from one sheet, or asked? | The survivor's value (an empty one filled from the first in board order), the others reported as dropped; scenes joined with ", " and synopsis and comments with a line break in board order; lengths and times summed; elements unioned (decision 35) |
| E-S14 | Merge strips across different sets, I/E or time of day: allowed, warned, or refused? Across sub-boards, or a strip in the Boneyard with one on a sub-board? | Allowed anywhere, nothing asked; the dropped values are listed |
| E-S15 | Merge strips with a banner or day break between them: does it stay where it was, move, or go? | Stays where it was; only the merged-away strips leave (decision 35) |
| E-S16 | Where does the merged strip land: at the topmost selected strip, at the survivor's place, or at the drop point? On other boards? | At the survivor's place on every board |
| E-S18 | Merge strips with several strips selected: which one keeps its sheet (the topmost, the first clicked, the one right-clicked)? In which order do the scenes join when boards order them differently? | Checked (note 18): the first selected keeps its sheet, fields and place; scenes and elements join in selection order, the survivor's first (decision 46). Shift-click order and a context-menu merge not tried |
| E-S19 | Paste strips: below the bottommost selected strip? With nothing selected, at the end of the sub-board? Are the pasted strips selected? | Checked in part (note 18): below the strip pasted on; a moved strip is selected, a pasted copy is not. Ours selects the pasted strips either way, on purpose (decision 52). With nothing selected (ours: the end of the picked sub-board) not tried |
| E-S20 | Cut strips: do they leave the board at once (and come back if never pasted), or stay until the paste? Cut elements on a sheet, the same? | Checked for strips (note 18): a cut strip stays, outlined, until the paste moves it, as ours (decision 38). Cut elements not tried |
| E-S21 | Copy and paste a scene strip: a new sheet (a duplicate), or the same sheet moved? Pasting a strip cut on another board? Where does a pasted copy go on the other boards? | Checked in part (note 18): a copy is a new sheet with the same scene number, as ours (decision 38). Pasting on another board and where the copy goes on other boards not tried (ours: the end of their Boneyards; a cut pastes copies there) |
| E-S22 | Strip designer: which fields does a new board's strip show, and how wide? Is a layout per board, or picked from a library the board and the one-line share? Cast as IDs or names? | Checked in part (note 18): the real app keeps named layouts in a list, each typed (strips, day breaks, shoot day), and its default strip has two lines with cast names and IDs. Ours has the typed library, two-line strips and per-board picks (decision 78); a new board's default stays one line. Whether a board picks from the list by reference, and the one-line's own layout, not seen |
| E-S23 | Delete on selected scene strips: one question for a mixed selection (scenes, banners, day breaks)? Do both Delete and Backspace act? Is the sheet gone from every board? | Checked (note 21): Delete Strip(s) in the context menu asks first, saying the selected strips and their breakdown sheets leave every board, with Cancel the default, as ours (decision 54). A mixed selection and Backspace not tried (ours: one question; both keys) |
| E-K2 | Insert Daybreak: its key (Cmd comma in older manuals), and does it go below or above the selected strip? Above the topmost or below the bottommost with several selected? | Below (⌘,), above with ⇧⌘,; the bottommost or the topmost (decision 55) |
| E-S24 | Breakdown sheet's planned date: where is it shown, and does a click (or a command) jump to the strip on the board? | The header line's place is a link; a click selects the strip on the Board tab (decision 58) |
| E-S25 | Strip colors: set per I/E value and per time of day value separately, or per pair? Per schedule or per board? Font color per strip type: scene, banner, day break, shoot day? | Per pair (an empty value matches any), first match wins, per schedule; text colors for scenes, banners and day breaks (decision 60) |
| E-S26 | Print or PDF the board: what does a printed strip show, in color? Options (selected strips, Boneyard, orientation, scale)? | The board's strip layout as columns, strips in their colors, day breaks with totals; selected only, Boneyard, portrait or landscape (decision 61) |
| E-S27 | Show or hide total strips, banners and shoot day strips: kept per board in the schedule, or a view setting? Does printing follow? | A view setting per viewer; printing follows (decision 64) |
| E-S28 | Resource counts on strips: which counts (cast, every element, per category, with quantities)? Shown as a column, a badge? | Two layout fields: cast count and element count, empty when none (decision 66) |
| E-S29 | Sheet number and stepping: is the sheet number free text or a number, set by hand or kept in order? Stepping by sheet or scene number: natural order, where do empty ones go? | Free text; natural order, empty ones last, board order among equals (decision 77) |
| E-S30 | Templates: what does a template keep (elements, boards and sub-boards, calendars, start dates, report and strip designs) and what does it clear? | Keeps the setup, elements, boards and calendars; clears sheets, strips, events and red flags (decision 77) |
| E-S31 | Header field order: per schedule or per user? Can a field be hidden? | Checked (note 19): part of the schedule's breakdown designer; fields are moved, removed and added back, in a grid of set widths. Ours keeps the order, hidden fields and widths in the schedule (decisions 77 and 101) |
| E-S32 | Advanced find: the operators offered per field type, page ranges, how AND and OR rows mix, what a find with no text lists | Nine operators, AND or OR for all rows, no text lists the sheets (decision 77). A blank replace in a one-value field leaves it empty, trimmed, so the value is unlinked (decision 189; the manual's reading, note 05, not seen in the real app) |
| E-S33 | Split screen: which panes (breakdown, board, calendar, images); can strips be dragged from one pane to the other, or onto calendar dates; is the split per user or per schedule? | Four panes, no drag between panes, per user in this browser (decision 80) |
| E-S34 | The strip designer's element quantity field (note 25): does it pick elements one by one or whole categories, does a picked element left off the board still count, and are the red flag and conflict switches per layout or per board? | Checked (note 27): whole categories, one per field, a new pick replacing the old; the picker lists categories only and its search finds their names; the strip shows `Cast Members (4)`, `Props (0)` with none. The switches sit beside the layout's name, so they belong to the layout, as ours. Not checked: whether an element left off the board counts; our reading leaves it out, as the cast count and the every-element count do (decisions 136 and 147) |
| E-E17 | The red flag date picker (note 25): does Shift-click pick a run from the last date clicked, and does Command-click on a picked date remove it? | Checked (note 27): Command-click on a picked date removes it; Shift-click adds the run from the last date picked (a Shift-click's end counts, a removing Command-click doesn't) and keeps the rest, without Command. Changed (decision 144) |
| E-S17 | Duplicate sheet: does the copy land after the original, or after the selection in the active sub-board? On other boards (end of the Boneyard, or after the original)? Is the scene number copied as is, or changed (12 to 12A)? | Checked (note 22): Duplicate Sheet (the New Sheet button's menu) puts the copy right after the original, in the same day, with the same scene number, selected, as ours: right after the original wherever it is on a sub-board, else at the Boneyard's end; every field copied as is. Other boards not tried |
| E-R7 | Report date range: one per sub-board for every report, or per report? Inclusive of both ends? | Checked (note 20): per report. Switching from the one-line to the calendar changed the range, the calendar reaching back to an event before the first shoot day, so each report works out its own from what it shows. Ours keeps one per report (decision 59), inclusive |
| E-R14 | Report designer: what does the day break choice "fewer" keep; how does the total strip count print; does the element category picker add columns to the one-line? | Day breaks all or none; a last total line; a column per picked category (decision 100) |
| E-R8 | Breakdown by day: what the number in brackets after an element counts; with quantities, the sum or the most on one sheet? | How many of the day's scenes list it, and `x3` for the most any one sheet needs (decision 59) |
| E-R9 | Vertical one-line: the cover page's cast list and the rows' order (board ID, or first appearance)? | Checked (note 23): no cover page; the left of the same page holds producer, director, assistant director and script date, then the cast as rows in board ID order; strips are columns with turned text and day breaks dark columns with the day's pages and time. Ours now prints the same way (decision 115), no ID last |
| E-D24 | Custom code letters (MMS 5): do they reach the grid, the summary and the exports alike? Several letters per code? | Everywhere the grid shows codes (tab, report, spreadsheet, MCP); 1 to 3 characters each, no two the same (decision 59) |
| E-C11 | Calendar: a strip dragged to another day lands where (end of the day, or at the drop among its strips)? Does the filter hide or dim other strips? Week start and date format per app or per schedule? | Checked in part (note 19): the week start and date format are app-wide settings, as ours; the calendar strip designer is part of the schedule (decision 102). Checked in part (note 27): the filter's category list holds the categories in use and its element list a check box per element under its category, with All, several checked; strips without a checked element are hidden, day headers and events stay. Changed: ours checks several categories and elements (decision 146). Where a dragged strip lands not seen (ours: the end of the day) |
| E-D25 | DOOD sort keys and directions; does a sort clear on leaving the tab or switching sub-board? | Checked in part (note 19): up to four keys, each a field and an order, from board ID, element name, first and last working day, worked and held days, with Clear, Cancel and Sort, as ours (decisions 59 and 98). Checked (note 27): ties keep board order, as ours; leaving the DOOD tab and coming back clears the sort and the Sort dialog opens empty. Changed: leaving the tab clears it, as a sub-board switch does (decision 145). Where blanks go not seen (ours: last) |
| E-L2 | Linked elements: does adding A bring B's links too (more than one level)? Does Retroactive add every linked element or only the new link? May any element link across categories, or only set and location ones? | Checked (note 25): links chain through every level, one way, one undo; ours now follows the whole chain with a cycle guard, retroactive too (decision 116). Any element may link to any other, as the reference app's search offers every category |
| E-L10 | A category excluded from the board: are its elements left out of the day out of days, or only off the strips and out of conflicts? | Kept in the day out of days, never marked as conflicts; off the strips' cast column (decision 62) |
| E-L11 | Daily quantity: does an element with no quantity count as one? Two sheets the same day with the box on: added twice? Where does the asterisk show (the day's total, the category's column)? | Checked in part (note 25): no quantity counts as one, as ours. Two sheets and the asterisk not seen (ours: added twice; on the day's total, decision 63) |
| E-L12 | Header fields in the Elements tab and the day out of days: is a value matched ignoring case? Do set and location rows get holds? | Checked (note 25): ignoring case, as ours, spelled as first met; no holds or drops (decision 62); every one-value header field since decision 118. The sheet itself now takes the existing spelling when a value is typed (note 27 check BRK-02; decision 155) |
| E-L13 | The default category list: is any category besides the cast built in (can't be deleted)? | Only the cast (decision 62; E-L5) |

## DOOD: holds, drops, events, holidays

| ID | Question | Proposed rule (engine default) | Quick test in MMS |
|---|---|---|---|
| E-D1 | How is the drop gap measured against the minimum? | Gap = calendar days strictly between the last work day before the gap and the next work day (weekends and other off days included). Drop when gap is at least the minimum. | ADA on Day 1 (Mar 1) and Day 11 (Mar 15): 13 calendar days between. Holds on, drops on. Set minimum 12, then 13, then 14. Note which minimums give WD/PW and which give holds. |
| E-D2 | One drop per element, or one per qualifying gap? | One drop only, at the first qualifying gap; later long gaps show holds. | ADA on Days 1, 11, 21, minimum 7. Do we see one drop (after Day 1) or two? |
| E-D3 | Do holidays and company travel days inside a gap count toward the minimum? | Yes, all calendar days count. | Repeat E-D1 with minimum 13 after marking Wed Mar 10 a holiday. Does the drop still happen? |
| E-D4 | Where do D and P sit, and how do days inside a drop look? | D joins the last work day (WD, or SWD if it is also the start). P joins the return day (PW, or PWF). Days inside the drop are blank and counted nowhere. | From E-D1: read the cells on Mar 1, Mar 2 to 12, Mar 15. Check the summary's worked and held. |
| E-D5 | Do holds skip company off days? | Yes. H on every working day strictly between first and last work day when holds are on; weekends stay grey and uncounted. | ADA on Thu Mar 4 and Tue Mar 9, holds on. Expect H on Fri Mar 5 and Mon Mar 8, weekend grey, held = 2, worked = 2. |
| E-D6 | Event on a day the element also works: which code shows, and is the day counted twice? | Checked (notes 17 and 18): the event's code, then the whole work code (`FTPW`, `TRSW`), a warning, counted once as worked, a conflict (decisions 43 and 47). | Travel event for ADA on Day 3 where ADA also works. Read cell and worked total. |
| E-D7 | What does "worked" include, and is there a grand total? | Worked = W days + TR + FT + R days + qualifying holidays (/) + qualifying move days (M). Held = H days. Keep all separate; MMS 5's "work + hold + company travel" total is not used. | Wed Mar 3 a holiday, so Day 3 moves to Thu Mar 4 and Day 4 is Fri Mar 5. ADA on Day 1 and Day 4, holds on. Read the Mar 3 cell (/) and the summary columns. Export the DOOD summary to xlsx and list every column. |
| E-D8 | Event on a hold day: does it replace H, and is it held or worked? | Event code replaces H; the day counts as worked, not held. | ADA on Days 1 and 5, holds on, fitting on Day 3. Expect FT on Day 3, held 2, worked 3. |
| E-D9 | What code does an "unavailable" event show, and does it count? | Shows a code of its own, counts nowhere. | Unavailable event for ADA on Day 3 between work days with holds on. |
| E-D10 | An event before the start date (or after the last day): does the DOOD add a date column for it? Does S move? | Columns extend to cover the event; S stays on the first strip day; the event counts as worked. | Fitting for ADA on Fri Feb 26, ADA works Days 1 to 2. Look for a Feb 26 column and the totals. (MMS 5 extended the range; see note 09.) |
| E-D11 | Does a travel day inside a long gap block the drop? | Yes: the gap is split at the event; each part is tested alone. | ADA on Days 1 and 11, minimum 7, travel on Mon Mar 8. Expect no drop: holds plus TR. |
| E-D12 | Are hold and drop switches per element across the schedule, or per sub-board? | Per element, schedule-wide; the DOOD is computed per sub-board. | Add a second sub-board, turn ADA's holds off while on it, switch back to Main and look. |
| E-D13 | Holiday inside the span with holds off: shown? counted? | Not shown, not counted. | E-D7 with holds off. |
| E-D14 | Default DOOD row order. | Order of first appearance on the sub-board (first work day), ties by board ID. | Give CLEO the lowest board ID but the latest first day; see where CLEO lands. |
| E-D15 | Strips below the last day break: do they count in the DOOD? | No: only strips that sit above some day break are scheduled. | Put BRUNO only on a strip below the last day break. Is BRUNO in the DOOD? |
| E-D16 | Element in two strips on the same day. | One W; worked counts the day once. | Two strips with ADA before one day break. |
| E-D17 | Company travel (move) day: does it take a day number, and how does it show? | No day number (strips skip it); M shown and counted as worked only under the E-D7 conditions. | Mark Wed Mar 3 as company travel, ADA on Day 1 and Day 4 (Fri Mar 5 after the reflow), holds on. |
| E-D18 | Element with events but no strip work. | Event codes only, no S, F, H; worked = event days. | Fitting for CLEO on Day 2, CLEO in no strip. |
| E-D19 | Category default changes: do they reach existing elements? | No (MMS 10). We add an explicit "apply to existing" action. | Turn holds on for Cast in category settings; open ADA. |

## Day breaks and eighths

| ID | Question | Proposed rule | Quick test in MMS |
|---|---|---|---|
| E-B1 | Auto day breaks by eighths: is the max inclusive, and is it greedy in board order? | Greedy in board order; add strips while the day total stays at or under the max; then break. | Five strips of 3/8, 5/8, 4/8, 4/8, 1 (one page). Auto by max 8 eighths. Expect days [3/8 + 5/8], [4/8 + 4/8], [1]. |
| E-B2 | A strip longer than the max. | It gets a day of its own; packing continues after it. | Strips 2/8, 1 2/8, 2/8 with max 8 eighths. Expect [2/8], [1 2/8], [2/8]. |
| E-B3 | Auto by shoot time: do banner durations count? Where do banners go? | Banners count their duration and stay in place, packed like strips. | Strips 3:00, 3:00, banner 1:30, strip 2:00; auto by 7:30. |
| E-B4 | Does auto replace manual breaks and cover the whole sub-board from the top? | Yes: all breaks removed, then inserted from the first strip. | Add two manual breaks mid-board, select a strip in the middle, run auto. |
| E-B5 | Two day breaks in a row (an empty day). | Allowed; it is a shoot day with a date and no work; elements show nothing. | Put two breaks after one strip. Check day numbers, dates, DOOD. Also check MMS 5's "multiple day breaks" meaning: does the empty day carry the previous strip's cast? |
| E-B6 | After auto breaks, is the last partial day closed with a break? | Yes, a break after the last strip. | Any auto run; look at the board's end. |
| E-B7 | Eighths entry and display. | Store integer eighths; 13 eighths becomes 1 5/8; never reduce 4/8; whole pages show without a fraction. | Type 0 and 13, then 2 and 0, then 0 and 4. Read the field and a day total. |
| E-B8 | Shoot time sums: format and odd input. | Integer minutes; h:mm display. Unvalidated text: we import digits only, flag the rest. | Type 1:30, 0:45, 90, 1.5 on four sheets in one day; read the day total. |
| E-B9 | Sort: banners to top, breaks removed? | Yes, as the manual says. | Sort by set on a board with breaks and banners. |
| E-B10 | Shoot day strip count vs day break number across sub-boards. | Day break number counts breaks in this sub-board; shoot day strip shows the same count. | Two sub-boards with 3 days each; read the shoot day strips in the second. |
| E-S4 | Scene number sort: natural? | Natural: 2, 9, 10, 10A, 10B, 11, A12 last. | Sheets numbered 1, 2, 10, 10A, 9, A1, 1A; sort by scene. |
| E-S1 | Name uniqueness for boards and sub-boards. | Unique within the schedule. | Try two sub-boards named "Unit B" in different boards. |
| E-S2 | Move To: where do moved strips land? | Checked for a sub-board (note 24): the top of it, the selection to the next strip where it was; ours since decision 129, the Boneyard the same way (not seen). | Move one strip to the Boneyard. |
| E-S3 | Merge strips: what happens to scene, pages, elements, header? | Scenes joined with commas, eighths summed, elements unioned, header from the first. | Merge sheet 3 (1 page, ADA) and sheet 4 (3/8, BRUNO). |

## Calendar and sub-boards

| ID | Question | Proposed rule | Quick test in MMS |
|---|---|---|---|
| E-C1 | Day N date. | Nth date on or after the start date that is a working weekday and not an off day, holiday or company travel day. | Standard setup plus a holiday Wed Mar 3 and company travel Thu Mar 4: Day 3 should be Fri Mar 5. |
| E-C2 | Start date on an off day. | Day 1 is the next working day. | Start Sat Feb 27. |
| E-C3 | Working week per sub-board: does the DOOD of each sub-board use its own? | Yes. | Sub-board B with a six-day week; compare DOODs. |
| E-C4 | Sub-board with a later start date: do day numbers restart at 1? Does a conflict appear by date? | Each sub-board numbers its own days from 1 and dates from its own start; conflicts compare dates. | Sub-board "Unit B" starting Mon Mar 8 with ADA on its Day 1; Main has ADA on Day 6 (Mar 8). Expect a conflict. |
| E-C5 | Adding a holiday mid-schedule reflows later days. | Yes, every later break moves one working day. | Mark Mon Mar 8 a holiday after breaks exist. |
| E-C6 | Day breaks parked in the Boneyard. | No date, no number, ignored everywhere. | Move a break to the Boneyard. |
| E-C7 | Off days set on the board's off days control vs the calendar's working week: same setting? | One per sub-board setting. | Change one, read the other. |

## Conflicts, red flags, elements, import, keys, exports

| ID | Question | Proposed rule | Quick test in MMS |
|---|---|---|---|
| E-E1 | Conflict scope. | Same element in strips on the same date in two sub-boards of the same board; other boards ignored. | ADA on Mar 1 in Main and in a sub-board of the same board starting Mar 1; then the same in a second board. |
| E-E2 | Do events ever cause conflicts? | An event on a work day does (note 17, A4; decision 43). Overlapping events: no, informational only. | Two overlapping travel events for ADA. |
| E-E3 | What makes an element "active"? | Any element on a scheduled strip not excluded from the board. | Exclude ADA from the board (category or element setting) and repeat E-E1. |
| E-E4 | Red flag clash. | Flag shows when a flagged element is on a strip scheduled within the flag's dates. | Flag ADA unavailable Mar 2; schedule ADA on Day 2. |
| E-L1 | Category defaults vs existing elements. | Same as E-D19. | Same. |
| E-L2 | Linked elements: automatic or asked? Retroactive? Removal? | Adding the main element adds the linked one silently; retroactive applies to existing sheets; removing the main leaves the linked one. | Link a prop to ADA, add ADA to a new sheet, then remove ADA. |
| E-L3 | Renumber skipping locked IDs. | Unlocked elements numbered from the start value in custom order, skipping numbers held by locked ones. | Lock CLEO at 2; renumber from 1. Expect ADA 1, BRUNO 3. |
| E-I1 | `.fdx` import: heading split, scene numbers, pages. | I/E, set and time of day split from the heading; scene number from the file if present, else sequential; pages from scene properties. | Import an `.fdx` written from an invented three-scene script in any editor (with and without scene numbers). |
| E-I2 | Our Fountain pagination vs real lengths. | Settled by the owner (decision 195): 56 lines a page, 7 lines an eighth, line positions rounded to the nearest eighth, minimum 1/8 (decision 9); the count is a user setting, 40 to 70. | Same invented script as `.fdx` from an editor; import into MMS; compare each scene's pages with ours (only to tune the default; no longer blocking). |
| E-I3 | Re-importing a revised script. | MMS adds new sheets (duplicates) rather than merging. | Import the same `.fdx` twice. |
| E-I4 | Speaking characters: extensions and dual dialogue. | Name without extensions; dual dialogue cues included. | Script with (V.O.), (CONT'D) and a dual dialogue block. |
| E-I5 | How a tagged `.fdx` links a tagged word to its category (IMP-04). | A text run carries `TagNumber`; a `TagData` block after the content lists `TagCategory` (`Id`, `Name`), `TagDefinition` (`Id`, `CatId`, `Label`) and `Tag` (`Number`, with `DefId` children). Runs we can't follow are skipped. | Tag a prop, a cast member and a vehicle in an invented three-scene script in a screenwriting app, save as `.fdx`, and look at the XML (owner question 8); import it into MMS too and list which categories it fills. |
| E-I6 | Re-importing a revised script into a schedule (BYD-01, beyond E-I3). | Ours merges: scenes matched by number then heading keep their sheet, tags and place; new scenes go to the Boneyard; missing ones are marked omitted. | Import a script, schedule it, then import a draft with one scene renumbered, one cut and one added; note what MMS does with each (to say where ours differs on purpose). |
| E-I7 | A scene a revised script no longer has (decision 91). | It stays on the board in its place, struck, and listed struck in reports, but adds no pages, time or screen time to its day and is no work for its elements (day out of days, conflicts, red flags, the element list). | Re-import a draft with one scene cut (or mark a scene omitted, if the app offers it); read the day's totals, the day out of days and the one-line. |
| E-K1 | Full shortcut list. | Checked (note 21): File New ⌘N, Open ⌘O, Open Shared ⇧⌘O, Save ⌘S, Save As ⇧⌘S, Close ⌘W; Edit the standard keys; Schedule Breakdown ⌘., Board ⌘U, Calendar ⇧⌘D, Reports ⌘P, the rest none. Ours takes them except Reports (⌘P stays Print, a platform convention; decision 106) | Open the shortcut list from Help and copy it into a private note (our words, keys only). |
| E-R1 | DOOD summary xlsx columns. | Separate counts per kind. | Export from the E-D7 schedule; list headers and one row. |
| E-R2 | One-line xlsx columns and day rows. | One row per strip, banner and day break. | Export from the standard setup. |
| E-R10 | One-line Excel with element quantities (10.14.14): which categories get columns (the report's shown ones, or all), how an element shows (ID, name, both) and how a quantity is written | Checked (note 20): the reference app follows the strip layout (a first column for day break and banner text, the layout fields in order, day break rows with end text, date, page total and time), every cell text. Ours does by default; decision 93's category columns follow, and the fixed columns stay an option (decision 103) | Tag a prop with quantity 3 on a ZTEST scene, export the one-line to Excel, list the headers and that row |
| E-R11 | Breakdown by day in Excel: one row per day and element, or a row per day with a column per category? Are quantities and the scene count there? | A row per day and element with the scene count, the day's quantity and a partial mark (decision 94) | Export the by-day breakdown of a ZTEST schedule to Excel; list the headers and the first day's rows |
| E-R12 | Excel "working formulas" (10.11): which cells are formulas (day totals, grand totals, DOOD totals), and are values cached for viewers that don't recalculate? | Checked (note 20): the reference app's one-line Excel has no formulas, every cell text. Ours keeps its working totals on purpose, each with its value cached (decision 95) | Open a one-line and a DOOD summary Excel from MMS in a spreadsheet app; click the totals and note which show a formula |
| E-R13 | Saved PDF metadata (10.11): which properties are set (title, subject, author, keywords, creator) and from what? | Checked (note 20): a fixed generic title, the system PDF library as creator and producer, no author, subject or keywords. Ours sets more on purpose (decision 96); nothing to change | Save a ZTEST report as PDF from MMS; in Preview, Tools > Show Inspector, note each property |
| E-R3 | Merged cells and per-field type in the report designer (RPT-15). | A field on a later line covers several of the first line's columns; each field may have its own size, bold and italic (decisions.md, entry 90). | In the report designer, merge two cells and set one field's size; note whether merging joins cells of one row or stacks fields, and whether sizes are steps or points. |

## Recording wishlist (priority order)

Screen recordings by the owner, with a key display on screen, invented data only, 1 to 3
minutes each, saved to the gitignored `/Screenshots/`. Priority follows engine risk: the
DOOD and day breaks drive every number a line producer checks.

1. **DOOD holds and drops**: turn on holds for a few cast, then drops with a minimum gap;
   move a strip so a gap crosses the minimum; read the totals. (E-D1 to E-D5)
2. **Events on the DOOD**: a travel day before the first shoot day, a fitting on a hold day,
   a rehearsal inside a long gap; read cells and totals. (E-D6 to E-D11)
3. **Calendar days**: start date, a holiday and a company travel day mid-schedule, watch the
   day breaks reflow and the DOOD codes change. (E-C1, E-C5, E-D7, E-D17)
4. **Auto day breaks**: by eighths with a strip bigger than the max, then by shoot time with
   a banner duration, then remove all. (E-B1 to E-B4)
5. **Sub-boards**: a second unit with a later start date, the same actor in both, the
   conflict marker, the Boneyard, duplicating a board. (E-C4, E-E1)
6. **`.fdx` import**: what comes in and where it lands; a second import of a revised draft.
   (E-I1, E-I3, E-I4)
7. **Breakdown by keyboard**: two scenes, header fields, quick add, new elements.
8. **Elements tab**: board IDs, lock, renumber, merge, move category, linking.
   (E-L1 to E-L3)
9. **Board building**: sorting (with lettered scene numbers), moving, banners, merging
   strips. (E-S3, E-S4, E-B9)
10. **Reports**: one-line, shooting schedule, DOOD summary; export PDF and Excel. (E-R1, E-R2)
11. **Saving**: autosave on and off, templates, recent files.
12. **Five minutes of how the owner actually schedules a show.**

Plus open-format exports from the ZTEST schedules above: DOOD summary and DOOD report (xlsx
and PDF), one-line (xlsx), board (xlsx), element list, breakdown report.
