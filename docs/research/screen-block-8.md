# Screen block 8 (2026-11-06)

About nine minutes on the owner's yes, on the signed 0.8.0 app from rehearsal run 42330735073
(commit 2d326f3), so what was tested is what would ship. Test files only, removed afterward.
Full-screen control from the Open dialog on: the system's panels ignore background control.

## Passed

| Check | Seen |
|---|---|
| D245 Details | Address, two lines of directions and offset dates saved; the file holds them. The real Open dialog starts in the schedule's folder and stores `images/...`, a relative path. |
| D245 on the sheet | A saved call sheet PDF has the map, address, directions and coordinates. With the picture gone: "Map not found" and its path, and the sheet still prints. |
| D244 | Offset dates at -05:00 moved the printed sunrise to 06:18 against the location's -04:00. |
| D246 Weights | "abc" refused; changed weights changed the suggestion's costs; Use Standard went back. |
| D246 search | Real Suggest: swapping won at 32 against the three rules' 33, 38 and 38. Apply moved 5 scenes; Undo put them back. |
| D247 | The swatch and its title on the board, the swatch in the board PDF, rename updating the heading and the swatch. |
| D248 | `off` in the Day Timeline: the department is off that day's sheet only and stays listed as `off`; clearing it brought it back. |
| Crash recovery (M2) | kill -9 mid-edit: the Recovered dialog on restart, the edit back, Save asking before it replaces the file. Changed outside: Merge, Keep Mine, Load Theirs. A locked file: refused, nothing written. |
| Layouts | Rename, then one click on Done: the dialog closed and the name was kept. |
| K185 on paper, `\|` | The rule after a field prints on every strip. |
| K187 on paper | The Monospaced banner prints in Menlo. |

## Found, and fixed for 0.8.0

1. The map printed in color while the notes and decision 245 said gray: the CSS filter doesn't
   reach the saved PDF. The filter and the claim went; a map reads better in color.
2. K185's `_` rule under a field didn't print: the printed strip's bottom line outranked it. Older
   than 0.8.0. Fixed in `print.css`, with a page test; checked in computed styles, not yet in a
   saved PDF from the real app.
3. Print Call Sheet's description left out locations and crew calls.

## Not reached

A physical printer (PDFs only); the reference app's E-S34, RPT-03 and RPT-10; the update and
both-chips tests; DOD-06, DOD-07 and BYD-10.
