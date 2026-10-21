# Call sheet data

Method Schedule writes the data a call sheet needs for one shoot day, as JSON or CSV, so
a production can pour it into its own call sheet template, spreadsheet or mail merge.
The app also prints it as a designed call sheet (Plan > Print Call Sheet..., below). There
is no sending (decisions.md, BYD-02 and BYD-03; inventory, "Competitor features MMS lacks").

Where to get it:

- the app: Plan > Export Call Sheet Data... (pick the day, JSON or CSV, then where to save);
- the command line: `method-schedule callsheet FILE DAY [--csv] [--out OUT] [--sub-board ID]`
  (the first board; its first sub-board unless `--sub-board` says);
- the MCP server: the `call_sheet` tool returns the JSON below (docs/MCP.md).

## The printed call sheet

Plan > Print Call Sheet... picks a shoot day (the selected strip's, at first), then
**Print...** or **Save as PDF...**. The sheet is Letter, black ink only so it copies cleanly:

- the opening: the title, company, director, producer and board; the shoot day out of how
  many and its date; the call time, largest on the page, and the wrap; then the day's
  length, pages, scene count, sunrise and sunset and where the sun is for;
- the scenes in shooting order: clock times, scene, I/E, the set with the synopsis and
  location under it, day or night, pages and cast IDs;
- the cast: ID, name, scenes, their call and wrap (their own where the Day Timeline gives
  one) and when they are on set (first start to last end);
- the other elements, a line per category, each element with its scenes;
- the notes.

A long day runs onto more pages with the column headings repeated and "Page X of Y" in the
footer. It prints exactly the data below, nothing more (decisions.md, entry 207).

## What goes in

- **The day**: the schedule's title and production info, the board and sub-board, the
  day number out of how many, its date (when the sub-board has a start date), the call
  time and wrap (when the day has a call time), the day's length (shoot times and banner
  durations added up) and its pages.
- **Sunrise and sunset**: at the first of the day's scenes whose location is placed on
  the map (Plan > Locations...), for the day's date, in that location's local time: its
  UTC offset, an hour more on the dates its daylight saving rule is in effect. See
  decisions.md, BYD-08 and entry 193.
- **Scenes**, in board order: scene number, I/E, set, day or night, pages, cast board IDs
  (names for cast without one), location, synopsis, shoot time, and start and end clock
  times from the running timeline (decisions.md, BYD-04).
- **Cast**, in board ID order: board ID, name, the scenes they are in, and when their
  first scene starts and their last ends.
- **Elements** of every other category, in category order, each with its board ID, name
  and scenes.
- **Notes**: each banner with its time and duration, the events on the date (with the
  elements they list), and the timeline's warnings (a long day, exterior day scenes after
  sunset, scenes with no shoot time).

Times are 24-hour local clock times. A time of 1440 minutes or more runs past midnight.
Without a call time, the start, end, call and wrap fields are empty.

## JSON

A single object. Keys appear in alphabetical order. Every number is an integer: times
in minutes after midnight (with the same time as `HH:MM` text in a key without
`_minutes`), page lengths in eighths (with the page text beside them).

| Key | Type | Notes |
|---|---|---|
| `format` | string | Always `method-schedule-callsheet`. |
| `version` | integer | `1`. |
| `title`, `company`, `director`, `producer` | string | The schedule's title and production info. |
| `board`, `sub_board` | string | Names. |
| `day`, `days` | integer | The day number and how many shoot days the sub-board has. |
| `date` | string or null | `YYYY-MM-DD`. |
| `call`, `call_minutes` | string, integer, or null | |
| `wrap`, `wrap_minutes` | string, integer, or null | Call plus length. |
| `length_minutes` | integer | |
| `eighths`, `pages` | integer, string | The day's total. |
| `sun` | object | `state` (`times`, `always_up`, `always_down` or `unknown`), `location`, `sunrise`, `sunrise_minutes`, `sunset`, `sunset_minutes`. |
| `scenes` | array | Each: `sheet` (id), `scenes`, `int_ext`, `set`, `time_of_day`, `eighths`, `pages`, `synopsis`, `location`, `cast` (array of strings), `shoot_minutes`, `start`, `start_minutes`, `end`, `end_minutes`. |
| `cast` | array | Each: `element` (id), `board_id`, `name`, `scenes` (array of scene numbers), `first`, `first_minutes`, `last`, `last_minutes`, `call`, `call_minutes`, `wrap`, `wrap_minutes` (their own where set, else the day's; null without a call time), `own_call` (whether either is their own, decision 231). |
| `elements` | array | Each category: `category` (id), `name`, `elements` (each `element`, `board_id`, `name`, `scenes`). |
| `notes` | array of strings | |

## CSV

One file, UTF-8 with a byte order mark (so Excel reads accented letters), records ending
in CRLF. It holds five tables in turn, each with its own header row and a blank line
between them:

1. `Field,Value`: Title, Company, Director, Producer, Board, Sub-board, Day (`3 of 20`),
   Date, Call, Wrap, Length, Pages, Sun at, Sunrise, Sunset.
2. `Start,End,Scene,I/E,Set,Day/Night,Pages,Cast,Location,Synopsis`: one row per scene;
   cast board IDs joined by `, `.
3. `ID,Name,Scenes,First,Last,Call,Wrap`: the cast, with each one's call and wrap.
4. `Category,ID,Name,Scenes`: every other element.
5. `Note`: one note per row.

Fields are quoted only when they hold a comma, a quote or a line break. Text that a
spreadsheet would run as a formula gets a leading `'` (as every export of ours does).

## Not included

A map or directions, weather, crew calls, per-person call times,
advance schedules for the next days, and sending the sheet to anyone. Each is a later
option; distribution needs a service and stays out (BYD-03).
