# `method-schedule mcp`: schedules for coding agents

`method-schedule mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server built
into the command line. It lets a coding agent (Claude Code, or any MCP client) find
schedule files, read them, change them and check them with typed tools: import a script,
fill in a breakdown sheet, tag elements, move strips, set day breaks, edit the calendar,
then read the shoot days, the day out of days, the one-line schedule and the budget
exchange file.

It works on **files**, not on the running app, and keeps nothing between calls. Every tool
names a schedule by its path and reads it from disk; a change writes the whole file back
atomically, as `method-schedule` itself does. Reads return the file's **revision**, and every change
must hand that revision back, so a change based on an old read is refused (see "Revisions
and conflicts").

The server is plain Rust inside the `method-schedule` binary: no network, no other programs run, no
new crates (it uses the `serde_json` that `method-schedule-format` already pins). Every change is
made with `method-schedule-core`, the same engine the app and the command line use, and checked
with `Schedule::check` before anything is written. The design is Method Budget's
(`docs/MCP.md` in that repo), ported (decisions.md, entry 22).

## Starting it

```sh
method-schedule mcp --root DIR [--root DIR...] [--read-only]
```

| Flag | Meaning |
|---|---|
| `--root DIR` | A folder the server may read and write in. At least one; repeat it for several. Paths in tool calls are relative to the first root, or absolute inside any root. |
| `--read-only` | Hide every tool that writes a file. They don't appear in `tools/list`, and calling one is an unknown tool. |

The server reads JSON-RPC messages from stdin, one per line, and writes one response per
line to stdout. stdout carries protocol messages only; a line per tool call (its name,
whether it worked, how long it took, never its arguments) goes to stderr. It exits when
stdin closes.

## Registering it

### Claude Code

```sh
# Every schedule under ~/Schedules, for every project (user scope).
claude mcp add --scope user method-schedule -- method-schedule mcp --root ~/Schedules

# Look but don't touch.
claude mcp add --scope user method-schedule-ro -- method-schedule mcp --root ~/Schedules --read-only
```

Use the binary's full path (`target/release/method-schedule` from a build) if `method-schedule` isn't on the
`PATH` Claude Code starts programs with. `claude mcp list` shows whether it connected;
`/mcp` in a session lists its tools.

For one project, a `.mcp.json` at the project root registers it for everyone who opens
the project. In this repository `.mcp.json` is ignored by git (`.gitignore`, with the
secrets), so here it is as text to copy:

```json
{
  "mcpServers": {
    "method-schedule": {
      "command": "/path/to/method-scheduling/target/release/method-schedule",
      "args": ["mcp", "--root", "/Users/me/Schedules"]
    }
  }
}
```

Add `"--read-only"` to `args` for a server that can't change anything. Other MCP clients
take the same shape (the key may be `mcpServers` or `servers`).

## Protocol

| | |
|---|---|
| Transport | stdio, newline-delimited JSON-RPC 2.0 (no embedded newlines, UTF-8). |
| Revisions | **2026-07-28** (per-request `_meta`, `server/discover`) and, through the `initialize` handshake, **2025-11-25**, **2025-06-18**, **2025-03-26** and **2024-11-05**: the same set as Method Budget. |
| Capabilities | `tools` only (`listChanged: false`): no resources, prompts, sampling or logging. |
| Methods | `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`, `notifications/cancelled`; `server/discover` for 2026-07-28 requests. |

**Version negotiation.** A client that sends `initialize` gets its own revision back when
it is one of the four above, else 2025-11-25, and is served under that revision until it
initializes again. A request whose `params._meta` carries
`io.modelcontextprotocol/protocolVersion` is served on its own under 2026-07-28: it must
also carry `io.modelcontextprotocol/clientCapabilities` (else `-32602`), any other version
gets `-32022 Unsupported protocol version` with the supported list in `data`, and its
result has `resultType: "complete"` and the server's name and version in
`_meta["io.modelcontextprotocol/serverInfo"]`. Under 2026-07-28, `server/discover` and
`tools/list` also carry the revision's caching hints: `ttlMs: 3600000` (the list is fixed
while the server runs, so a client may keep it an hour) and `cacheScope: "public"` (it is
the same for every caller); earlier revisions have no such fields. A request with neither (and no handshake
yet) gets `-32602` saying so; `ping` always works.

**Results.** A tool result has two text blocks, a one or two sentence summary for a person
and the result as compact JSON, and the same JSON as `structuredContent` (revisions from
2025-06-18 on; older ones get the text only). Tools declare no output schema; the fields
are listed below.

**Conventions.** Every number is an integer. Page lengths are eighths of a page in keys
named `eighths` (`11` is 1 3/8 pages), with the same length as text under `pages`
(`"1 3/8"`) where it helps reading. Times are whole minutes (`minutes`, `shoot_minutes`)
or seconds (`screen_seconds`); sunrise and sunset are minutes after midnight. Dates are
`YYYY-MM-DD` strings, or null. Everything is named by its stable integer id (sheets,
elements, categories, boards, sub-boards, strips), which never changes; events, which
have no id yet, by their place in the calendar's list.

**Boards.** Tools that work on one sub-board take `board` and `sub_board` (ids), both
optional: the first board, and its first sub-board, by default.

**Errors.** Problems with the message itself are JSON-RPC errors: `-32700` (not JSON, or a
line longer than 8 MB), `-32600` (not a request), `-32601` (unknown method), `-32602` (bad
params, unknown tool, a writing tool on a read-only server), `-32603` (a tool failed
unexpectedly). Everything a model can fix is a **tool error** instead: a result with
`isError: true`, the message as its text and `structuredContent: {"error": {"kind",
"message"}}`, where `kind` is one of:

| Kind | Means |
|---|---|
| `invalid_input` | The arguments don't fit the tool's schema, or a value is wrong (a date that doesn't exist, a file that isn't a schedule). |
| `not_found` | No such file, sheet, element, board, sub-board or strip. |
| `refused` | The change isn't allowed as asked: the engine refused it (a name or board ID already used, a banner sent to the Boneyard), or the result would fail `Schedule::check`, or a file exists, or a file is too large. |
| `conflict` | The file changed on disk since the read the change is based on. Nothing was written. |
| `path_refused` | The path is outside the roots, hidden, a symlink to write through, or has the wrong extension. |
| `io` | The disk said no. |

Arguments are checked against the tool's `inputSchema` before it runs: unknown
properties, wrong types (a float where an integer goes), values out of range and missing
required ones are refused with the property's name.

**Cancellation.** A request cancelled with `notifications/cancelled` before the server
reaches it never runs. One cancelled while it runs gets no response; if it was a change,
it has already been written.

## Revisions and conflicts

- **Revision.** Every read and every change returns the file's `revision`: FNV-1a 64 of
  its bytes as 16 hex digits, the same fingerprint as the budget exchange file's
  `source_hash`. It changes with every write. It notices any change; it is not a security
  hash.
- **Every change takes the revision it is based on** (`revision`, required). The change
  is refused with a `conflict` error, and nothing is written, when the file's bytes no
  longer have that revision: another program, a person in the app, or an earlier call
  saved it since. Read it again, look at what changed, then make the change again with
  the new revision. A change returns the new revision, so a chain of changes passes each
  result's revision to the next call.
- **Just before the rename** the file is read once more and compared with the bytes the
  change started from, so a save that lands while the change is being made is not lost.
- **Every change is checked.** It is made on a copy with `method-schedule-core`; the copy must pass
  `Schedule::check` and come back unchanged through the file format's strict reader, or
  the change is refused and nothing is written. A change that changes nothing writes
  nothing (`changed: false`, same revision).
- **Writes are atomic.** A temporary file in the same folder, flushed, then renamed over
  the schedule, keeping its permissions. A reader never sees half a file. A new file is
  linked into place, which fails rather than replace a file that appeared meanwhile.

## Tools

74 tools; `--read-only` keeps the 22 that read.

| Group | Tools |
|---|---|
| Reading | `list_schedules`, `read_schedule`, `get_sheet`, `list_elements`, `days`, `dood`, `one_line`, `budget_exchange`, `check`, `diff`, `explain` |
| Find and conflicts | `find`, `conflicts` (read), `replace` (writes) |
| Files and scripts | `new_schedule`, `import_script` |
| Breakdown | `new_sheet`, `delete_sheet`, `set_sheet_fields`, `merge_sheets`, `duplicate_sheet`, `tag_element`, `untag_element`, `set_quantity` |
| Elements | `add_element`, `rename_element`, `set_element_rules`, `set_board_id`, `set_element_details`, `merge_elements`, `move_element`, `delete_element`, `renumber_elements`, `add_category`, `set_category`, `delete_category` |
| Board | `move_strips`, `insert_day_break`, `delete_day_break`, `insert_banner`, `set_banner`, `auto_day_breaks`, `remove_day_breaks`, `sort_strips` |
| Calendar | `set_start_date`, `set_day_kind`, `set_weekly_off`, `add_event`, `delete_event` |
| Red flags | `red_flags` (read), `set_red_flag`, `add_red_flags`, `delete_red_flag`, `set_red_flag_type_color`, `add_standard_red_flag_types` (write) |
| Reports | `report`, `report_list` (read), `set_production`, `set_dood_codes`, `set_prep_wrap` (write) |
| Beyond the reference app | `timeline`, `call_sheet`, `rest_report`, `drift`, `suggest_order`, `locations` (read), `revise_script`, `set_call_time`, `set_location`, `set_turnaround`, `lock_schedule`, `pin_strips`, `push_past_wrap`, `apply_order` (write) |

Every tool that works on one schedule takes `path`. Every reading result has `path` and
`revision`; every change returns `path`, `changed` and `revision`, plus what is listed.

### Reading

**`list_schedules`** `{folder?, recursive?}`: the schedule files under every root (or
`folder`): each `.mtsc` and `.mtst` (template) file, and each `.json` file whose first
bytes name the schedule format. Hidden files and folders and symlinks are skipped; at most 2,000 files, 8 folders
deep. Returns `files` (`path`, `bytes`) and `truncated`.

**`read_schedule`** `{path, offset?, limit?}`: `title`, `canonical` (whether the file is
byte for byte what this build writes), `template` (whether it is a template, which no
tool changes), `counts` (categories, elements, sheets, boards),
`eighths` and `pages` (all sheets), `categories` (`id`, `name`, `elements`), `boards`
(`id`, `name`, `boneyard` strip count, `sub_boards` with `id`, `name`, `start`, `end` (the
last shoot day's date), `shoot_days`, `strips`, `unscheduled`, `events`), and `sheets` in
creation order from `offset` (`id`, `scenes`, `int_ext`, `set`, `time_of_day`, `eighths`,
`pages`, `elements` count), 500 by default, 5,000 at most.

```json
{"name": "read_schedule", "arguments": {"path": "Harbor/harbor.mtsc"}}
```

**`get_sheet`** `{path, sheet}`: every header field (`number`, `scenes`, `int_ext`, `set`,
`time_of_day`, `sunrise`, `sunset`, `eighths`, `pages`, `shoot_minutes`, `synopsis`,
`location`, `script_pages`, `script_day`, `screen_seconds`, `unit`, `sequence`, `episode`,
`comments`), `elements` (ids, in the order added), `quantities` (`element`, `count`,
`daily`: the elements given a quantity other than one, counted daily), `images` (paths), `element_details` (`id`, `category`,
`name`, `board_id`) and `places`: on each board, `sub_board` (null for the Boneyard),
`index`, `strip` and `day` (the shoot day it falls on, if any).

**`list_elements`** `{path, category?}`: `categories` (`id`, `name`, `auto_board_id`,
`exclude_from_board`, `daily_quantity`, `properties` (custom property names, in order),
`multi_line` (those whose values take several lines),
`defaults`: `holds`, `drops`, `min_drop_gap`) and `elements` (`id`,
`category`, `category_name`, `name`, `board_id`, `board_id_locked`, `exclude_from_board`
(its own setting), `excluded` (the same since decisions.md entry 120), `rules`, `notes`,
`properties` (`name`, `value`, the ones with a value), `links` (the elements adding it
to a sheet adds), `images` (paths, relative to the schedule's folder or absolute),
`sheets`: how many sheets it is on). `category` (a name) keeps one.

**`days`** `{path, board?, sub_board?}`: `start`, `days` (each `number`, `date`, `eighths`,
`pages`, `minutes`, `screen_seconds`, `day_break` (the strip closing it) and `strips`),
`unscheduled` (strips after the last day break), `boneyard` and `calendar` (`weekly_off`,
`work_days`, `off_days`, `holidays`, `moves`, `events` with `index`, `kind`, `label`,
`name`, `from`, `to`, `elements`). A strip is `{strip, kind: "sheet", sheet, scenes,
int_ext, set, time_of_day, eighths}`, `{strip, kind: "banner", text, minutes, style}` or
`{strip, kind: "day_break"}`. A banner's `style` is `{bold, color, tall, size, align,
font}` (decisions.md, entries 65, 87 and 187): `color` one of `yellow`, `orange`, `red`, `pink`,
`purple`, `blue`, `teal`, `green`, `gray`, any color as lowercase `#rrggbb` (entry 154),
or null for the board's banner color; `tall`
two lines high; `size` `small`, `normal` or `large`; `align` `left`, `center` or `right`;
`font` `usual` (the board's), `serif`, `sans` or `mono`.
Strip ids here are what `move_strips`, `delete_day_break` and `set_banner` take.

**`dood`** `{path, board?, sub_board?, category?, header?}`: the day out of days (the sub-board
needs a start date). `rows`, in the engine's order (board ID, then name): `element`,
`category`, `board_id`, `name`, `start`, `finish`, `worked`, `held` and `cells`, every
date from the row's first day to its last with its `code` (`SW`, `W`, `WF`, `SWF`, `SWD`,
`PW`, `H`, `TR`, `FT`, `R`, `FTH`..., `/` for a holiday that counts, `M` for a company
travel day that counts, and empty for a day that counts nowhere). An event on a day the
element works shows the event's code and then the whole work code (`FTW`, `FTPW`,
`TRSW`, `FTWF`), counts once as worked, and carries `conflict: true` (not for an element
left off the board, decision 120). The codes are always in the usual letters, so a client
reads them one way; the schedule's own letters (`set_dood_codes`) are in `report`'s
`dood_grid`. `summary`: `element`,
`category`, `board_id`, `name`, `work`, `hold`, `start`, `finish`, `total` (work plus
hold). The counting rules are in decisions.md, entries 10, 11, 43 and 47. `header`
(`set`, `location`, `unit`, `episode`, `int_ext`, `time_of_day`, `script_day` or
`sequence`) gives a row per value that header field takes
on the sheets instead, worked out from them (decisions.md, entry 62); its `element` ids
are made up for the call.

**`one_line`** `{path, board?, sub_board?, columns?}`: `days`, each with `number`, `date`, `eighths`,
`pages`, `minutes` and `lines`: scenes (`kind: "scene"`, `scenes`, `int_ext`, `set`,
`time_of_day`, `eighths`, `pages`, `synopsis`, `cast`: Cast board IDs, or names where an
element has none, and `elements`: each category the schedule's one-line report shows, in
category order, as `category` and `elements` (`element`, `board_id`, `name`, `quantity`
on the sheet), the same as the one-line export's element columns; `omitted: true` on a
scene a script revision left out, which counts nowhere) and banners (`kind: "banner"`, `text`, `minutes`, `style` as in
`days`). With `columns` (`layout` or `fixed`) it adds `table`, the one-line
export's first sheet: `columns` (header names) and `rows` (cells as text, numbers or
null). `layout` follows the report's strip layout as the reference app's Excel does (a
first column for day break and banner text, the fields as strips show them, a day break
row closing each day, the category columns, then `kind` and `eighths`); `fixed` has the
stable `day` to `text` columns (decision 103).

**`budget_exchange`** `{path, board?, sub_board?, generated?}`: `exchange`, the budget
exchange file exactly as `method-schedule budget` writes it (docs/EXCHANGE.md), as JSON.
`generated` is the date to stamp it with; left out, the file has no `generated` field, so
the result depends on the schedule alone.

**`check`** `{path}`: `ok`, `problem` (why the file doesn't read or doesn't hang together,
or null) and `canonical`. Unlike the other tools, a file that fails is a result, not an
error.

**`diff`** `{path, other}`: what changed from `path` (the older schedule) to `other` (the
newer), matched by id (docs/MERGE.md). Returns both files' `revision` (`revision`,
`other_revision`) and `changes`, in the diff's order, each with `kind` (`added`,
`removed`, `changed`, `moved`), `subject` (`kind`: `title`, `setup` (production info,
report list, code letters), `strip_colors`, `layouts` (the layout library), `category`,
`red_flag_type`, `element`, `red_flag` (with `element` and `red_flag`), `sheet`, `board`,
`sub_board`, `calendar` or `strip`, with the ids it needs), `fields`
(`field`, `before`, `after`, as text) and `text`, one plain sentence. Strips that changed
place are listed one by one.

**`explain`** `{path, element | name, category?, board?, sub_board?}`: why one element's
day out of days counts on a sub-board are what they are (it needs a start date). Name the
element by id, or by `name` (ignoring case) and `category` when the name is in more than
one. Returns `element`, `name`, `category`, `rules`, `start`, `finish`, `work`, `hold`,
`total`, what work is made of (`work_days`, `event_days`, `holidays`, `moves`), `drop`
(`after`, `before`, `days`, or null), `gaps` (each `after`, `before`, `days`, `outcome`:
`dropped`, `too_short`, `another_gap_dropped`, `drops_off` or `holds_off`, and `text`),
`days` (every date of the row: `date`, `code`, `counts`: `work`, `hold` or `nowhere`,
`reason`, `shoot_day`, `sheets`, `events`, `text`) and `summary` (sentences). The codes
and counts are the `dood` tool's.

### Find, replace and conflicts

The engine side is `method_schedule_core::find` and `method_schedule_core::conflicts`
(decisions.md, entry 28); the command line has the same three as `method-schedule find`,
`replace` and `conflicts`.

**`find`** `{path, text, field?, match_case?, whole_word?, board?, sub_board?,
conditions?, any?, limit?}`:
every place `text` is found, ignoring case unless `match_case`; `whole_word` means the
match doesn't run on into letters, digits or `_` on either side. `field` narrows it to
`sheet` (every header field), one header field (`number`, `scenes`, `int_ext`, `set`,
`time_of_day`, `synopsis`, `location`, `script_pages`, `script_day`, `unit`, `sequence`,
`episode`, `comments`), `element` (names), `category` (names), `banner` (text) or `event`
(names). With `sub_board` (and `board` when it isn't found by itself) the search covers
one sub-board: its sheets and banners (scheduled or not), its events, and the elements on
those sheets and events with their categories. Results are in board order: each board's
sub-boards' strips in turn (a sheet's header fields the first time it is met, banners),
each sub-board's events after its strips, sheets met only in a Boneyard, then categories,
each followed by its elements. Returns `count`, `truncated` and `results` (at most
`limit`, 500 by default): `number` (from 1), `kind` (`sheet`, `element`, `category`,
`banner`, `event`), `field` (a header field, `name` or `text`), the ids that name the
place (`sheet`; `element`; `category`; `board`, `sub_board` and `strip`; `board`,
`sub_board` and `event`, the event's index), `text` (the whole field) and `context` (a
short line such as `Scene 12 set: CABIN`).

An advanced search (decisions.md, entry 77) adds `conditions`, up to 20 rows of
`{field, op, value?}`: `field` a header field, `pages`, `shoot_time` or `screen_time`;
`op` one of `contains`, `not_contains`, `is`, `is_not`, `starts_with`, `empty`,
`not_empty`, `greater`, `less` (numbers compare as numbers, text naturally, so scene 12
is after 9); `value` pages as `2` or `1 3/8`, a shoot time as `h:mm`, a screen time as
`m:ss`. The rows join by AND, or by OR with `any: true`. With conditions only the
header fields of the sheets that meet them are searched, and `text` may be empty to
list each such sheet once.

**`replace`** `{path, revision, text, with, field?, match_case?, whole_word?, board?,
sub_board?, conditions?, any?, all | picks}`: replaces what the same search finds by `with` (empty removes
the text) in every result (`all: true`) or in the results numbered `picks` (find's
numbers, from 1). Element and category names are trimmed afterwards, as a rename trims
them; a place whose text comes out the same isn't a change, so the same text changes
nothing and writes nothing. Refused, with nothing written, when a rename would give two
elements of one category (or two categories) the same name ignoring case, would leave a
name empty, or would put a line break in a one-line field (every field but `synopsis`
and `comments`). The cast category can be renamed like any other. Returns `changes`, each with
the place's `kind`, `field` and ids, `place` (`Scene 4 set`), `before` and `after`.

```json
{"name": "replace", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "text": "NET LOFT", "with": "BOATHOUSE",
  "field": "set", "picks": [1, 2]}}
```

**`conflicts`** `{path, board?}`: elements scheduled on the same date in two or more
sub-boards of one board; other boards are other arrangements of the same sheets and are
never compared. Dates come from each sub-board's start date, calendar and day breaks, as
the day out of days has them: work days only (not holds), strips below the last day
break not scheduled, elements left off the board left out. Returns
`count` (every conflict), `conflicts` (each `date`, `element` with `id`, `name`,
`category`, `board_id`, and `bookings`: `sub_board`, `sub_board_name`, `day`, `sheets`
with `id` and `scenes`), `event_on_work` (conflicts too: a travel day, fitting or
rehearsal on a date the element works on the same sub-board, behavior note 17; each
`date`, `element`, `sub_board`, `sub_board_name`, `day` and `event` with `index` and
`name`), `not_compared` (sub-boards with no start date) and, apart, `event_clashes` with
`event_clashes_note`. **Event clashes are informational, not conflicts** (engine check
E-E2). A clash is an element with two or more events on a date anywhere on the board, or
an event in one sub-board and strip work in another; each lists `events` (`sub_board`,
`sub_board_name`, `index`, `name`) and the day's `bookings`.

### Files and scripts

**`new_schedule`** `{path, title?}`: an empty schedule (one board, Board 1, with one
sub-board, Main, the Cast category and our default categories after it, decisions.md
entry 62) in a new file. The file must be `.mtsc` and must
not exist; nothing is ever replaced. Returns `board` and `sub_board`.

**`import_script`** `{path, revision, script, category?, tags?, lines_per_page?}`: a Fountain (`.fountain`) or
Final Draft (`.fdx`) file inside the roots, as the app's Import Script brings it in: one
sheet per scene at the end of every board's Boneyard, with the speaking characters as
elements of `category` (by default the cast category, whatever it is called; created
when missing; matched by name, new names created). With `tags` true (`.fdx` only), each
scene's production tags come in too, as elements in categories of the same names (a
tag category called Cast or Cast Members is the cast; others are created when missing;
decisions.md, IMP-04). A Fountain script is paginated at `lines_per_page` lines a page,
40 to 70, default 56 (7 lines an eighth; decision 195); a Final Draft file keeps its own
lengths. Returns `sheets` (the new ids in script order) and `eighths` (their total).

```json
{"name": "import_script", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "script": "drafts/harbor-light.fountain"}}
```

### Breakdown

**`new_sheet`** `{path, revision, board?, after?}`: an empty sheet, as the Breakdown tab's
New Sheet: right after sheet `after` when that sheet is on a sub-board of the board,
otherwise at the end of the Boneyard; every other board gets it at the end of its
Boneyard. Returns `sheet`, `board`, `sub_board` (null for the Boneyard) and `index`.

**`delete_sheet`** `{path, revision, sheet}`: deletes the sheet and its strip on every
board; its elements stay in the schedule, day breaks and banners stay where they are.

**`set_sheet_fields`** `{path, revision, sheet, fields}`: changes only the fields given.
Text fields (`number`, `scenes`, `int_ext`, `set`, `time_of_day`, `synopsis`, `location`,
`script_pages`, `script_day`, `unit`, `sequence`, `episode`, `comments`) take a string
(empty clears one); `sunrise` and `sunset` take 0 to 1439 or null; `eighths` the page
length; `shoot_minutes` and `screen_seconds` a whole number or null. A one-value field
(`set`, `location`, `int_ext`, `time_of_day`, `unit`, `sequence`, `episode`,
`script_day`) takes the spelling another sheet already has when it matches ignoring
case, as the app's Breakdown tab does (decisions.md, entry 155).

```json
{"name": "set_sheet_fields", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "sheet": 12,
  "fields": {"synopsis": "Kit mends a net.", "eighths": 11, "shoot_minutes": 90}}}
```

**`merge_sheets`** `{path, revision, survivor, others}`: merges sheets into `survivor`
(merge strips). The survivor keeps its id and its place on every board; the others are
deleted and their strips leave every board (banners and day breaks stay). In the order
given, the survivor first and then `others` as listed: scenes joined by `", "`, synopses
and comments by a line break, elements unioned; page lengths, shoot times and screen
times summed. Every other field keeps the survivor's value, or the first in that order
when the survivor's is empty; the others' values come back as `dropped`. Refused: the
survivor among `others` (`refused`), a sheet twice (`refused`), an unknown sheet
(`not_found`). Returns `survivor`, `merged` (the order given), `scenes`, `eighths`, `pages`, `combined` (`field`,
`before`, `after` for each field that changed), `elements_added` (`id`, `name`,
`category`) and `dropped` (`sheet`, `field`, `value`). decisions.md, entries 35 and 46.

```json
{"name": "merge_sheets", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "survivor": 12, "others": [13]}}
```

**`duplicate_sheet`** `{path, revision, sheet}`: copies a sheet, every field and element,
to a new sheet (to split a scene). On each board where the original is on a sub-board the
copy goes right after it; where it is in the Boneyard, at the Boneyard's end. Returns
`original`, `sheet` (the new id) and `places` (`board`, `sub_board` or null, `index`).

**`tag_element`** and **`untag_element`** `{path, revision, sheet, element}`: puts an
element on a sheet (once) with every element its links reach, through every level (a
cycle adds nothing twice), or takes it off (its quantity
too; linked ones stay); the element stays in the schedule. `tag_element` returns
`added`, the elements newly on the sheet.

**`set_quantity`** `{path, revision, sheet, element, count, daily?}`: how many of an
element on the sheet the sheet needs (1 to 99,999), and whether it counts in the day's
total for its category (`daily`, true by default). The daily category totals add
quantities for a category with `daily_quantity` on; one with `daily` false is left out
and the day's total is partial (decisions.md, entry 63).

### Elements

**`add_element`** `{path, revision, category, name, board_id?, sheet?}`: adds an element to
a category by name, creating the category when the schedule doesn't have it, with the
category's default rules and, when it numbers automatically, the next board ID. Names are
unique in a category ignoring case, and so are board IDs. Returns `element`, `category`,
`category_created` and `board_id`.

**`rename_element`** `{path, revision, element, name}`: a new name, unique in the category
ignoring case (`refused` otherwise). Returns `element` and `name`.

**`merge_elements`** `{path, revision, survivor, others}`: merges elements of one category
into `survivor`, which keeps its name, board ID and rules; every sheet and event that
listed any of them lists the survivor once, and their links, quantities and red flags
follow; the others are deleted. Elements of different categories are `refused`, as in the
reference app (engine check E-L6); `others` with nothing but the survivor is
`invalid_input`. Returns `survivor`, `merged` and `sheets` (how many hold the survivor now).

**`move_element`** `{path, revision, element, category}`: to another category (an id). Its
name and board ID must be free there, or `refused` (decision 45). It keeps its board ID and
lock; one without an ID keeps none, also in a category that numbers automatically (that is
for new elements only, decision 140). Its
custom property values are lost. Returns `element`, `category` and `board_id`.

**`delete_element`** `{path, revision, element, unlink?}`: it leaves every sheet and event.
An element linked to others, or linked from them, is `refused` unless `unlink` is true,
which drops its links both ways first.

**`renumber_elements`** `{path, revision, category, start?}`: board IDs from `start` (1 by
default) in `list_elements` order (board ID, then name). Locked IDs stay and their numbers
are skipped; IDs with letters stay; every other element gets the next free number.
Returns `category` and `elements` (`element`, `board_id`).

**`add_category`** `{path, revision, name}`: a category at the end; a name already used,
ignoring case, is `refused`. Returns `category`. **`delete_category`** `{path, revision,
category}`: deletes a category with no elements; the cast category can't be deleted.

**`set_element_rules`** `{path, revision, element, holds?, drops?, min_drop_gap?}`: the day
out of days switches; only those given change. Returns `rules`.

**`set_board_id`** `{path, revision, element, board_id, locked?}`: a board ID (letters
and digits, at most 12, like `12` or `12A`; unique in the category) or null to clear
it, and the lock that keeps it through renumbering.

**`set_element_details`** `{path, revision, element, notes?, properties?, links?,
retroactive?, exclude_from_board?}`: the notes; values of the category's custom properties (`[{name, value}]`,
named ignoring case, an empty value clears one; a name the category lacks is refused);
`links`, every element it links to (adding it to a sheet adds them; removing a link takes
nothing off a sheet), with `retroactive` also putting them on every sheet that holds it
now; `exclude_from_board` leaves this element off the board on its own (no ID on strips
or the one-line, never a conflict); its own setting decides (decisions.md, entry 120).
Returns `notes`, `properties`, `links`, `exclude_from_board`, `excluded` (the same) and
`sheets_gained`.

**`set_category`** `{path, revision, category, name?, position?, auto_board_id?,
exclude_from_board?, daily_quantity?, properties?, multi_line?}`: a category's settings;
`name` renames it (unique ignoring case; the cast category too), `position` moves it
among the user categories (from 0; the cast category stays first); `properties` is the
whole list of custom property names in order (a name kept, even in another case, keeps
its values; one left out loses them); `multi_line` names the properties whose values take
several lines (the whole list; the others take one line). A new schedule's cast starts
with eight properties (decisions.md, entry 119). `exclude_from_board` is the setting new
elements of the category get; existing elements keep their own (entry 120).

### Board

**`move_strips`** `{path, revision, board?, strips, to_sub_board, index}`: moves strips of
one board, in their current board order, to `index` (from 0, counted in the destination
before the move; past the end means last) in sub-board `to_sub_board`, or in the Boneyard
when it is null (sheet strips, and day breaks, which park there with no day; a banner is
refused). Up to 5,000 strips per call.

**`insert_day_break`** `{path, revision, board?, sub_board?, index}`: a day break at
`index`. Returns `strip`.

**`delete_day_break`** `{path, revision, board?, strip}`: deletes a day break, joining the
days either side. A sheet's strip or a banner is refused.

**`insert_banner`** `{path, revision, board?, sub_board?, index, text, minutes?, bold?,
color?, tall?, size?, align?}`: a banner at `index` (text up to 200 characters; minutes,
at most 1,440, count toward the day's time; the style parts as in `days`, plain when left
out). Returns `strip`, `text`, `minutes` and `style`.

**`set_banner`** `{path, revision, board?, strip, text?, minutes?, bold?, color?, tall?,
size?, align?}`: changes a banner (banners are only ever on a sub-board); what is left out is
kept, and `color: null` goes back to the board's color. Anything but a banner is refused.
Returns the banner as `insert_banner` does.

**`auto_day_breaks`** `{path, revision, board?, sub_board?, by, limit}`: replaces every day
break on the sub-board. `by` is `eighths` (pages: 40 is 5 pages), `minutes` (shoot time)
or `seconds` (screen time). Strips fill a day in board order and the day closes as soon as
its total reaches or passes `limit` (decisions.md, entry 12). Returns `shoot_days`.

```json
{"name": "auto_day_breaks", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "by": "eighths", "limit": 40}}
```

**`remove_day_breaks`** `{path, revision, board?, sub_board?}`: removes every day break on
the sub-board; its strips stay in order, unscheduled. Returns `removed`, how many went.

**`sort_strips`** `{path, revision, board?, sub_board?, keys}`: sorts the sub-board's
strips by one to four `keys`, each `{field, descending?}` with `field` one of `scene`,
`int_ext`, `set`, `time_of_day`, `location`, `pages`, `script_day`, `unit`, `sequence`,
`episode`, each field once (decisions.md, entries 32 and 122). Banners go to the top in
their order, day breaks are removed (break the days again after), empty values come
last, ties keep their order and scene numbers sort naturally. Returns `sorted`, the scene
strips on the sub-board.

### Calendar

**`set_start_date`** `{path, revision, board?, sub_board?, date}`: the sub-board's first
possible shoot day, or null to clear it. Dates, the day out of days and the budget
exchange need it.

**`set_day_kind`** `{path, revision, board?, sub_board?, date, kind}`: makes a date `work`,
`off`, `holiday` or `move` (a company travel day), dropping what it was; on a weekly day
off, `work` marks it back to working (decisions.md, entry 21). Returns `kind`.

**`set_weekly_off`** `{path, revision, board?, sub_board?, days}`: the weekdays the
sub-board never shoots on, each `mon` to `sun` (Saturday and Sunday by default; empty for
none). At least one weekday stays working, so all seven are refused. Dates already marked
keep their marks, and later shoot days move to the next working dates. Returns `days`,
Monday first.

**`add_event`** `{path, revision, board?, sub_board?, kind, label?, name?, from, to,
elements?, color?}`: `travel`, `fitting` and `rehearsal` count in the day out of days;
`unavailable`, `camera_test`, the post-production kinds (`post_production`,
`visual_effects`, `dubbing`, `sync_sound`, `music_recording`, `picture_editing`,
`sound_editing`, `sound_mixing`, `titles_graphics`, `trailer`, `color_correction`) and `other` (with `label`
saying what it is) show on the calendar only and are never conflicts (decisions.md, entry
109). `from` and `to` are inclusive and cover at most 366 days; `elements` are element
ids, any number; `color` is one of `red`, `orange`, `yellow`, `green`, `teal`, `blue`,
`indigo`, `purple`, `pink`, `brown` (none: the kind's usual look). Returns `index`. The
calendar's events (in `calendar` and the sub-board reads) carry `color` (null when none)
and `counts`.

**`delete_event`** `{path, revision, board?, sub_board?, index}`: deletes the event at
`index` (from 0, in the calendar's list); the events after it move up one place, so read
the calendar again before deleting another. A place with no event is `not_found`. Returns
`index` and `deleted`, the event's name, or its kind in words when it has none.

### Red flags

Red flags are dates an element can't (`hard`) or would rather not (soft) work, each of a
type from the schedule's own list (decisions.md, entry 33). They are warnings: nothing is
refused because of one. The engine side is `method_schedule_core::redflags`.

**`red_flags`** `{path, board?, sub_board?}`: returns `types` (`id`, `name`, `color`,
`builtin`: a standard type's event kind key, else null; decision 113), every
`red_flags` entry in the schedule (`id`, `element`, `element_name`, `category`, `type`,
`type_name`, the dates as `from` and `to`, or `not_before`, or `not_after`, the others
null, `dates` in words, `note`, `hard`), and the `clashes` on one sub-board, ordered by
date: each `date`, `element`, `element_name`, `red_flag`, `type`, `type_name`, `hard`,
the shoot `day` (null when the element only has an event that date), `sheets` (`id`,
`scenes`) and counted `events` (`index`, `name`). Work dates come from the sub-board's
start date, calendar and day breaks as for `conflicts`; travel, fitting and rehearsal
events count with or without a start date. `undated` is true when the sub-board has no
start date, so only events were checked.

**`set_red_flag`** `{path, revision, element, red_flag?, type | type_name, from and to |
not_before | not_after, note?, hard?}`: adds a red flag to an element, or replaces the
flag `red_flag` (it stays on its element). `type_name` adds the type when the schedule
doesn't have it (names ignore case). Exactly one way of giving the dates; a range that
ends before it starts is refused. Returns `red_flag` as `red_flags` lists it.

```json
{"name": "set_red_flag", "arguments": {"path": "harbor.mtsc",
  "revision": "6bc908bae34318d7", "element": 8, "type_name": "Unavailable",
  "from": "2027-03-02", "to": "2027-03-03", "note": "Exam week", "hard": true}}
```

**`add_red_flags`** `{path, revision, elements, type | type_name, dates | from and to |
not_before | not_after, note?, hard?}`: one red flag of one type for each element and
each run of dates (behavior note 23). `dates` lists dates picked one by one; following
dates join into one range (`2027-03-02`, `2027-03-03` is one flag). All or nothing: an
unknown element, an element given twice, no elements or no dates are refused. Returns
the new `red_flags`, element by element.

**`add_standard_red_flag_types`** `{path, revision}`: adds the standard red flag types
the schedule lacks (decision 113), one per event kind but `other`, at the end of the
list; a type of the schedule's own with the same name becomes the standard one. New
schedules start with them; older files keep their own list until asked. Standard types
can be renamed and recolored but not deleted. Returns `added`.

**`delete_red_flag`** `{path, revision, red_flag}`: deletes one red flag; its type stays.

**`set_red_flag_type_color`** `{path, revision, type, color}`: the color a red flag type's
marks are drawn in, `#rrggbb`, or null for none (the usual red; decisions.md, entry 84).
`red_flags` lists each type's `color`.

### Reports

The reports and settings added with the report options (decisions.md, entry 59).

**`report`** `{path, board?, sub_board?, kind, sort?, no_holds?, elements?, from?, to?}`:
`report`, the data of one report. `kind`:
- `dood_grid`: rows (`element`, `board_id`, `name`, `category`, `cells` with `date` and
  `code` in the schedule's code letters, `work`, `hold`, `start`, `finish`, `total`).
  `sort` takes up to four keys (`board_id`, `name`, `start`, `finish`, `work`, `hold`,
  or the reference app's words `first_work`, `last_work`, `worked`, `held`, each with
  `:desc` for descending; stable, blanks last); `no_holds` works the grid out
  as if no element had holds.
- `breakdown`: each sheet on the sub-board in board order: `scene` (`sheet`, `scenes`,
  `int_ext`, `set`, `time_of_day`, `eighths`, `pages`), `slot` (`day`, `date`, null when
  after the last day break) and `elements` by category (`category`, `elements` as
  `ID NAME`).
- `breakdown_day`: each shoot day: `day`, `date`, `scenes`, `eighths`, `minutes` and
  `elements` by category, each element once with how many of the day's scenes list it
  (`scenes`) and the day's quantity (`needed`): for an element of a category that uses
  daily quantities, each of the day's sheets' quantity of it added up (one for a sheet
  that gives none), leaving out one a sheet marks not to count daily (`partial` true);
  1 for any other element, which counts once a day (decisions.md, entries 63 and 91).
- `element_detail`: each element on the sub-board's sheets: `element`, `text`,
  `category`, `scenes` (each with its `slot`) and `work_dates`.
- `vertical`: `cast` (the rows, by board ID) and `days`, each with its totals
  (`eighths`, `minutes`, for its day break column) and each scene with the `rows` it
  lists.
- `multi_unit`: `dates`, each with the `units` shooting that date (`sub_board`, `name`,
  `day`, `eighths`, `minutes`, `scenes`), and `undated` sub-boards. `elements` keeps only
  days with a scene listing one of them.
- `calendar`: each date from `from` to `to` (default: the first to the last shoot day):
  `date`, `kind`, `shoot_day`, `scenes`, `eighths`, `minutes`, `events` (indexes).

**`report_list`** `{path}`: `reports`, the report list in order (`name`, `kind`, `from`,
`to`, `landscape`, `synopsis`, `cast_names`, `page_per_day`, `page_break` (`none`,
`day`, `week`), `shoot_days`, `day_breaks`, `total_count`, `element_columns` (category
ids), `layout` (`id` and `name` of the named strip layout picked, or null), `banners`,
`categories`, `elements`, `colors`, `icons`, `header` (the header layout's name),
`text`, `columns`, `no_holds`, `weekdays_only`; each of `columns` is a field's name, or
for a column the report designer placed or styled an object with `field`, `line` (1 to
4), `span` (the first line's columns it covers), `align`, `bold`, `italic` and `size`,
null for the report's text size), `header_layouts` (each `id`, `name`, `include`,
`first_page_only`, `bold`, `italic`, `align`, `size` and `rows`, each row a list of item
names, `{"text": ...}` for custom text; decision 99), `production` (`title` and every
production field below) and `dood_codes`.

**`set_production`** `{path, revision, company?, director?, producer?,
unit_production_manager?, assistant_director?, art_director?, set_dresser?, script_date?,
production_number?, preparer?, timing?, episode?, custom_1? .. custom_4?}`: sets the given
fields (trimmed, at most 200 characters; empty clears). Returns every field.

**`set_dood_codes`** `{path, revision, default?, start?, work?, finish?, hold?, drop?,
pickup?, travel?, fitting?, rehearsal?, holiday?, company_move?}`: the day out of days
code letters, each 1 to 3 letters, digits or `/`, no two the same; `default` starts from
the usual letters. Returns `codes`, all of them in order.

**`set_prep_wrap`** `{path, revision, board?, sub_board?, prep_start?, wrap_end?}`: the
sub-board's prep start and wrap end (null clears; a key left out stays as it is), which
the budget exchange carries. Wrap can't end before prep starts.

### Beyond the reference app

The features beyond the reference app (decisions.md, IMP-04 and BYD-01 to BYD-09). Clock
times are minutes after midnight with `HH:MM` text beside them; a time of 1440 or more
is after midnight.

**`timeline`** `{path, board?, sub_board?, day?}`: `days`, each shoot day (or only
`day`) with `call`, `wrap` and their minutes, `length_minutes`, `sun_location`, `sun`
(`state`: `times`, `always_up`, `always_down` or `unknown`, with `sunrise` and `sunset`)
and `entries` (each strip's `kind`, `sheet`, `text`, `start`, `end`, `untimed`,
`warnings`), and the day's `warnings`. Without a call time, times count from the call.
Each day's `past_wrap` (decision 192) says what runs past its wrap, the call plus the
long day limit: `wrap_by` and its minutes, `strips` (whole strips in board order, from
the first after the day's first strip that ends past `wrap_by` to the day break; empty
when nothing does), `names`, `next_day` (null on the last day) and `text`.

**`call_sheet`** `{path, board?, sub_board?, day}`: the call sheet data for one shoot
day, exactly as docs/CALLSHEET.md describes the JSON.

**`rest_report`** `{path, board?}`: `short`, each rest under its category's turnaround
(`element`, `name`, `category`, `from`, `wrap`, `to`, `call`, `rest_minutes`,
`minimum_minutes`, `text`), `unchecked` (work days with no date or call time) and
`turnarounds`.

**`drift`** `{path, lock?}`: without `lock`, `locks` (`name`, `board`, `made`, `scenes`);
with it, `actors`, each with `before` and `after` day counts and the work days `added`,
`dropped` and `moved` (`sheet`, `from`, `to`). A work day is a date, or `Sub-board day N`
when the sub-board has no start date.

**`suggest_order`** `{path, board?, sub_board?}`: a strip order for the sub-board, not
applied: `order` (every strip id), `moved` and `pinned` (counts of scene strips: those
that would change place, and those that stay put, pinned, omitted or one part of a scene
spanning days), `groups`, and `before` and `after` counts (`shoot_days`, `cast_days`,
`location_changes`, `red_flag_clashes`).

**`locations`** `{path}`: every sheet location with its sheet count and, when on the
map, `latitude`, `longitude` and `utc_offset` as text and `daylight_saving` (`none`,
`us_canada` or `eu`; decision 193).

**`revise_script`** `{path, revision, script, tags?, lines_per_page?}`: merges a revised
draft (see decisions.md, BYD-01; `lines_per_page` as for `import_script`): `matched`, `changed` (`sheet`, `by_heading`, `fields` with
`before` and `after`, `added`, `no_longer_speaking`, `restored`), `added` and `omitted`
sheet ids, and `text`, the report in lines.

**`set_call_time`** `{path, revision, board?, sub_board?, day, call}`: `call` as `HH:MM`
or null to clear.

**`set_location`** `{path, revision, name, latitude?, longitude?, utc_offset?,
daylight_saving?, remove?}`: degrees as decimals (`34.0522`, `-118.2437`) or with a
compass letter (`118.2437W`), up to six decimals; `utc_offset` as `-7`, `+05:30` or
`UTC-08:00` (default: the longitude's hour), the standard offset when there is a daylight
saving rule; `daylight_saving` one of `none`, `us_canada` or `eu` (left out, the
location's rule stays). `remove` takes the name off the map.

**`set_turnaround`** `{path, revision, category?, minimum?, long_day?}`: a category's
turnaround minimum (`12` or `10:30`; null clears; give `category` and `minimum`
together) and the long day limit (null: 12 hours).

**`lock_schedule`** `{path, revision, board?, name, remove?}`: saves where every
scheduled scene of the board is now under `name` (returns `scenes`, how many), or deletes
the lock.

**`pin_strips`** `{path, revision, strips, pinned?}`: pins (or unpins) scene strips for
the order suggestion. Returns `count`, how many changed.

**`push_past_wrap`** `{path, revision, board?, sub_board?, day, strips?}`: moves what
runs past the day's wrap (`timeline`'s `past_wrap`) to the start of the next shoot day,
in order; day breaks and call times stay. With `strips`, refused unless they are still
what would move; refused on the last day or when nothing runs past. Returns `moved`
(strip ids), `names` and `to_day`.

**`apply_order`** `{path, revision, board?, sub_board?}`: works the suggestion out again
and applies it. Returns `moved`, `before` and `after`.

## Safety model

The server treats the client as able to make mistakes and schedule files as untrusted
data.

- **Folders.** Every path must resolve inside a root. Relative paths start at the first
  root. An existing file is resolved with every symlink followed and must still be inside
  a root, so a link that leads out is refused; nothing is ever written through a symlink,
  even one that stays inside. Nothing below a root whose name starts with `.` (`.git`,
  `.ssh`, `.claude`, hidden files) is read or written.
- **Extensions.** Schedules are read and changed as `.mtsc` or `.json`, and created only
  as `.mtsc`; templates (decision 77) are read as `.mtst` and never changed, nor is a
  `.json` file marked as a template, with a sentence saying so; scripts are read only as
  `.fountain` or `.fdx`. A `.json` file is changed
  only when it already reads as a schedule, so the server can't be pointed at a config
  file, a script to run or a git hook.
- **Nothing is replaced or deleted.** `new_schedule` never replaces a file, and no tool
  removes one. A change replaces a schedule only with the same schedule changed.
- **Outside changes win.** A change needs the revision of the read it is based on and is
  refused when the file no longer has it (see "Revisions and conflicts").
- **Checked, atomic writes.** Every change goes through `method-schedule-core`, then
  `Schedule::check` and the strict reader, before an atomic write that keeps the file's
  permissions.
- **Read-only mode** removes every writing tool from the list, and calling one fails.
- **Integers only.** Every number in every schema is an integer with bounds; dates are
  parsed strictly.
- **Nothing runs, nothing leaves.** The server never starts a program, opens a socket or
  reads anything but the files named in calls (and the folders `list_schedules` walks).
- **Schedule text is data.** Titles, headings, synopses, names and notes come back as JSON
  strings; summaries quote schedule text (cut to 60 characters, control characters
  removed) and never present it as an instruction. The server's instructions to the client
  say so too.
- **Limits** (below) bound memory and time for every input.
- What it doesn't defend against: someone who can already write inside a root (they can
  change schedules without the server), or a file swapped for a link between the check and
  the write.

## Limits

| What | Limit |
|---|---|
| One protocol message | 8 MB |
| A schedule or script file read or written | 64 MB (the command line's own limit) |
| Free text (synopsis, comments) | 10,000 characters |
| A name, title or board ID | 200 characters |
| A path | 4,096 bytes |
| Strips in one `move_strips` | 5,000 |
| Results from one `find`; picks in one `replace` | 5,000 |
| Sheets listed by one `read_schedule` | 5,000 |
| Files listed by `list_schedules` | 2,000, 8 folders deep |
| An event | 366 days, 1,000 elements |
| Red flags added by one `add_red_flags` (elements times runs of dates) | 10,000 |
| A day break limit | 600,000 |

The `.fdx` reader has its own limits on size, nesting and elements (decisions.md, entry
13).

## Tests

`crates/method-schedule-cli/src/mcp/tests.rs` runs a scripted session through the server's real read
loop on in-memory streams (initialize, tools/list, a read, a change with the right
revision, the same revision again refused as a conflict, paths outside the root refused,
a parse error), and checks read-only mode, a change that would fail `Schedule::check`
leaving the file unchanged, every tool on the Harbor Light fixture (`diff` against a copy
taken before the edits, `explain` against the `dood` summary), symlinks, oversized files,
both protocol eras and cancellation. `find_replace_and_conflicts` covers the three find
and conflict tools: numbering and context, options and scope errors, a picked replace
with the revision, the stale revision refused, a name collision refused with nothing
written, the same text changing nothing, and conflicts across two sub-boards with an
undated one. `red_flags_set_list_and_delete` adds a flag by type name, lists its clash,
replaces it with an open-ended one, checks each refusal (two ways of giving dates, half a
range, no type, a missing type or flag, a reversed range, a stale revision) writes
nothing, and deletes it. `merge_and_duplicate_sheets` checks each merge refusal writes
nothing, merges three sheets given out of board order (the report's order, sums,
dropped sets and added elements), then duplicates the survivor and finds the copy right
after it. `banners_insert_list_and_restyle` inserts a styled banner, finds its style
where `days` lists it, changes part of the style keeping the rest, and checks an unknown
color and a sheet's strip are refused.

## See also

docs/FORMAT.md (the file format), docs/EXCHANGE.md (the budget exchange file), docs/MERGE.md
(diff, merge and explain), docs/APP.md (the app's own saving safety), and decisions.md,
entries 22, 25, 33 and 35.
