# Schedule file format

The open, documented file format Method Schedule saves schedules in. It is plain JSON, made to be
read and written by other tools and to diff cleanly in version control. The reference
implementation is `crates/method-schedule-format` (`to_string` and `from_str`).

Status: **version 0**, before the first release. Until version 1 ships, a change may break
older files. From version 1 on, changes are additive only. The file extension waits for the
product name (decisions.md, entry 4); it will look nothing like other tools' extensions.

## At a glance

- UTF-8, pretty-printed JSON, 2-space indent, `\n` line endings, one trailing newline.
- Keys always appear in the order documented below. The same schedule always produces the
  same bytes.
- **Every number is a JSON integer.** Page lengths are eighths of a page, times are whole
  minutes or seconds. Dates are strings, `YYYY-MM-DD`. No floats anywhere.
- Fields at their default value are left out, so files stay small and diffs stay focused.
  One rule for every key added since the first version: **an explicit default is
  refused**. A file that writes a key at its default (`"span": 1`, a column `width` equal
  to its field's, `"template": false`, a `size` of `normal`, an `align` of `auto`,
  `"long_day_minutes": 720`, an empty `day_look`, `uses`, `plan` or named layout `look`,
  a report's `text` of `normal`, `header` of `full` or `orientation` equal to its kind's)
  is an error, since the writer would leave the key out and the file would not write back
  byte for byte. The reader holds every key to this, old and new: it writes back what it
  read and refuses a file that differs (decisions.md, entry 133), so a key written as
  `null`, at its default (`false`, `[]`, `{}`, `""`, `0` where those are the default) or
  in another spelling than the writer's is refused with the key's place in the file.
- Readers are strict: an unknown field, a wrong type, an impossible date or a key given
  twice in one object is an error, and so is a schedule that doesn't hang together (see
  [Consistency](#consistency)). Layout files are read the same way. Fuzz tests hold the
  reader to this: a mutated file is refused, or it reads into a schedule that passes
  `check` and writes back as the file was (decisions.md, entry 173). The one place for
  keys this version doesn't know is an [`extensions`](#extensions) object.
- Everything carries a stable integer `id` that survives edits and moves, so two versions
  of a schedule can be compared and merged by identity, not by position.

## Top level

```json
{
  "format": "method-schedule",
  "version": 0,
  "schedule": { ... }
}
```

| Field | Type | Notes |
|---|---|---|
| `format` | string | Always `"method-schedule"` (decisions.md entry 27). Anything else is "not a schedule file". |
| `version` | integer | Format version. A reader refuses a version newer than it knows. |
| `template` | boolean | `true` marks a template (File > Save as Template; decisions.md, entry 77). Left out on a schedule; `false` is refused. A template is a schedule with no sheets, no strips, no events, no red flags and no plan locks, call times, pinned strips or omitted scenes, and a file marked as one that holds any is refused; opening one starts a new, untitled schedule from it. A command line write to a template keeps it marked. Its working extension is `.mtst`. |
| `schedule` | object | The [schedule](#schedule). |
| `extensions` | object | Additions from other apps, kept and written back untouched ([Extensions](#extensions)). Left out when there are none. |

### Extensions

Another app, or a newer Method Schedule, can add what it needs to a file without the strict
reader refusing it: an `extensions` object, the last key of the top level, the
[schedule](#schedule), and each [category](#category), [red flag type](#red-flag-type),
[element](#element), [sheet](#sheet), [board](#board) and [sub-board](#sub-board)
(decisions.md, entry 224). What's inside is the other app's: any JSON, kept as read
(its keys in sorted order) and written back on the same object, never looked at. The
convention is one key per app, named for it in reverse-domain form
(`"org.example.sync": {...}`), so two apps don't collide.

- An empty `{}` or a `null` is refused (the writer leaves the key out), and so is anything
  but an object. Every other unknown key is still refused.
- Kept by the object's id. Deleting the object drops them; duplicating it doesn't copy
  them. A merge keeps both sides', ours first, but not those on objects only the other
  side added (they get new ids in the merge).
- A file without any reads and writes back byte for byte as before; an older Method
  Schedule refuses a file that has one, as it refuses any key it doesn't know.

## Schedule

| Field | Type | Default | Notes |
|---|---|---|---|
| `title` | string | | |
| `production` | [production](#production) | none | The production info report headers show. |
| `next_id` | integer | | The next id to hand out. Ids are never reused, across all kinds, and every id in the file is below this. |
| `categories` | array of [category](#category) | empty | In display order. |
| `red_flag_types` | array of [red flag type](#red-flag-type) | empty | In display order. |
| `strip_colors` | [strip colors](#strip-colors) | none | The colors the user set for strips. Left out when none are set. |
| `calendar_hidden` | array of strings | empty | What the Calendar hides on each date (its View switches; decisions.md, entry 111): `day_labels`, `pages`, `time`, `scenes`, `events`, `strips`, `banners`, `red_flags`, each once, written in that order. Left out while it shows everything, so older files read and write back byte for byte. |
| `sheet_fields` | array of strings | the default order | The order the Breakdown tab lays the sheet's header fields out in (decisions.md, entry 77), as keys: `number`, `scenes`, `int_ext`, `set`, `time_of_day`, `shoot_time`, `synopsis`, `location`, `pages`, `script_pages`, `script_day`, `screen_time`, `unit`, `sequence`, `episode`, `comments`, `sunrise`, `sunset` (that is the default order). Fields left out follow in the default order. Left out at the default. |
| `calendar_fields` | array of strings | `scene`, `set`, `elements`, `cast` | What a strip shows on a Calendar date, in order: 1 to 13 [strip layout](#strip-layout) field names, each once, never `spacer` or `text` (decisions.md, entry 102). Left out at the default. |
| `sheet_hidden` | array of strings | empty | The header fields the Breakdown tab hides (the breakdown designer, decisions.md, entry 101), as `sheet_fields` keys, in that list's default order. A hidden field's values stay on every sheet and in the file. Left out when none is hidden. |
| `sheet_widths` | object | empty | A width per header field the user set, key to `narrow`, `medium` or `wide`; a field left out keeps its usual width. Keys in alphabetical order. Left out when none is set. |
| `sheet_rows` | array of integers | empty | How many of the shown header fields each of the Breakdown tab's rows holds, in order (decisions.md, entry 186); fields past them make one more row. Each 1 or more, together at most 18. Left out at the usual rows (six, five and the rest). |
| `layouts` | array of [named layout](#named-layout) | empty | The layout library, in its order (decisions.md, entry 78). |
| `one_line_layout` | integer | none | The named strip layout the one-line report uses; none: the board's. |
| `elements` | array of [element](#element) | empty | |
| `sheets` | array of [sheet](#sheet) | empty | Breakdown sheets, in creation order. |
| `boards` | array of [board](#board) | | At least one. |
| `reports` | array of [report](#report) | the default list | The Reports tab's list, in order, each report with its options. Left out while it is the default list. |
| `header_layouts` | array of [header layout](#header-layout) | the three it starts with | The report header layouts (decisions.md, entry 99). |
| `dood_codes` | [day out of days codes](#day-out-of-days-codes) | the usual letters | |
| `plan` | [plan](#plan) | none | Planning data for the features beyond the reference app. Left out while empty. |

### Production

Every field is a string of at most 200 characters, `""` by default and then left out;
the object is left out when every field is empty. The fields after `producer` came with
behavior note 19 (decisions.md, entry 97); a file without them reads and writes back byte
for byte.

| Field | Notes |
|---|---|
| `company` | The production company. |
| `director` | |
| `producer` | |
| `unit_production_manager` | |
| `assistant_director` | |
| `art_director` | |
| `set_dresser` | |
| `script_date` | Text, as typed (not read as a date). |
| `production_number` | |
| `preparer` | Who prepared the schedule. |
| `timing` | |
| `episode` | |
| `custom_1` to `custom_4` | Four free fields. |

### Report

One entry of the report list (decisions.md, entry 59): a named report of a kind, with its
options. Names are unique, ignoring case, and at most 200 characters. A kind the list
lacks is shown at its end with its default options, so the list always has every kind.

| Field | Type | Default | Notes |
|---|---|---|---|
| `name` | string | | |
| `kind` | string | | `one_line`, `shooting`, `breakdown` (by strip), `breakdown_day`, `dood`, `elements`, `element_detail`, `totals`, `calendar`, `vertical`, `multi_unit`, `conflicts` or `red_flags`. |
| `from` | date | none | Only shoot days (or calendar dates) from this date on. |
| `to` | date | none | Only up to this date, inclusive; not before `from`. |
| `orientation` | string | the kind's | `portrait` or `landscape`. Left out at the kind's default: landscape for `dood`, `calendar` and `vertical`, portrait for the rest. |
| `synopsis` | boolean | `true` | Show each scene's synopsis. |
| `cast_names` | boolean | `false` | Cast by name instead of board ID. |
| `page_per_day` | boolean | `false` | Each shoot day starts a new page. |
| `page_break` | string | none | `week`: each week starts a new page (a day whose week differs from the day before's, by the user's week start). A page per day stays `page_per_day`; the two never come together (decisions.md, entry 100). |
| `shoot_days` | boolean | `true` | The line heading each shoot day. |
| `day_breaks` | boolean | `true` | The line ending each shoot day with its totals. |
| `total_count` | boolean | `false` | A last line with the report's scene count and pages. |
| `element_columns` | array of integers | empty | Category ids: a column each after the scene columns, listing each scene's elements of that category (at most 10). |
| `layout` | integer | none | A named strip layout's id (`layouts`): the scene columns follow it unless `columns` are set. |
| `banners` | boolean | `true` | Show banners. |
| `categories` | array of integers | empty | Category ids: only these categories (empty: all). |
| `elements` | array of integers | empty | Element ids: only these elements (empty: all). |
| `colors` | boolean | `false` | Strip colors on scene lines. |
| `icons` | boolean | `true` | Red flag marks on scene lines. |
| `header` | string or integer | `full` | The [header layout](#header-layout) it prints with: `compact` or `title` for the second and third of the three layouts the list starts with (ids 2 and 3), as files wrote them before header layouts; any other layout by its id (4 on). |
| `text` | string | `normal` | `small`, `normal` or `large`. |
| `columns` | array of strings or [report columns](#report-column) | empty | The scene fields shown, in order, as in a [strip layout](#strip-layout) (`scene`, `set` ...). Empty: the kind's default, or the board's strip layout for the one-line reports. A column that is only its field is written as the field's name; one the report designer placed on a later line, merged across columns or styled is an object (decisions.md, entry 90). |
| `no_holds` | boolean | `false` | The day out of days as if no element had holds (and so no drops). |
| `weekdays_only` | boolean | `false` | The calendar with Monday to Friday only. |

#### Header layout

One entry of `header_layouts` on the schedule (decisions.md, entry 99; behavior note 19).
The list is left out while it is the three layouts it starts with: `Full` (id 1),
`Compact` (2) and `Title only` (3). Saved, it holds at least one and at most 50 layouts,
ids unique, names unique ignoring case. A report whose layout is gone prints with the
list's first.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | 1 or more. New layouts count on from the highest id, never below 4. |
| `name` | string | | At most 200 characters. |
| `hidden` | boolean | `false` | The layout prints no header. |
| `every_page` | boolean | `false` | Atop every page, rather than the first only. |
| `bold` | boolean | `false` | |
| `italic` | boolean | `false` | |
| `align` | string | left | `center` or `right`. |
| `size` | string | `normal` | `small` or `large`. |
| `rows` | array of arrays | | Up to three rows, each up to 12 items: `space`, `title`, `report_name`, `today`, `board`, `sub_board`, `start_date`, any [production](#production) field's key, or `{"text": "..."}` (one line, at most 100 characters). |

#### Report column

A report's column that is more than its field (the report designer, decisions.md, entry
89): the same lines and merged cells as a [strip layout](#strip-layout) column, and a text
size of its own. Its width follows the board's strip layout. Every field but `field` is
left out at its default, and an object that sets nothing else is refused (write the
field's name).

| Field | Type | Default | Notes |
|---|---|---|---|
| `field` | string | | A scene field, as in a strip layout. |
| `second_line` | boolean | `false` | On the second line. |
| `line` | integer | none | On the third or fourth line: `3` or `4`. |
| `span` | integer | `1` | How many of the first line's columns a field on a later line covers (a merged cell). |
| `align` | string | the field's | `left`, `center` or `right`. |
| `bold` | boolean | `false` | |
| `italic` | boolean | `false` | |
| `size` | string | the report's | `small`, `normal` or `large`; left out to follow the report's `text`. |

The report's columns are refused for what a strip layout's are: no field on the first
line, a field twice, a later line covering more columns than the first line has, a field
on the first line covering more than one.

### Day out of days codes

The code letters the day out of days shows (decisions.md, entry 59). Each field is a
string of 1 to 3 letters, digits or `/`, left out at its default; the object is left out
when every one is. No two may be the same, ignoring case.

| Field | Default |
|---|---|
| `start` | `S` |
| `work` | `W` |
| `finish` | `F` |
| `hold` | `H` |
| `drop` | `D` |
| `pickup` | `P` |
| `travel` | `TR` |
| `fitting` | `FT` |
| `rehearsal` | `R` |
| `holiday` | `/` |
| `company_move` | `M` |

### Plan

Planning data for the features beyond the reference app (decisions.md, the entries for
IMP-04 and BYD-01 to BYD-09). Every field is left out while empty, and the object is
left out when all are, so older files read and write back byte for byte. Ids that no
longer exist (a deleted day break, category or sheet) are ignored when read and dropped
by the next plan edit; they are never an error.

| Field | Type | Default | Notes |
|---|---|---|---|
| `locations` | array of [location](#location) | empty | Sheet location names placed on the map, for sunrise and sunset. |
| `call_times` | array of `{day, minutes}` | empty | A shoot day's call time: `day` is the id of the day break strip that closes the day, `minutes` the time after midnight (0 to 1439). One per day. |
| `long_day_minutes` | integer | 720 | The day length past which the timeline warns (1 to 1440). Left out at 12 hours. |
| `turnarounds` | array of `{category, minutes}` | empty | The shortest rest between wrap and next call for a category's elements (1 to 2880 minutes). One per category. |
| `locks` | array of [lock](#lock) | empty | Schedule locks, to report drift against. |
| `pinned` | array of integers | empty | Scene strip ids the strip order suggestion leaves in place. |
| `omitted` | array of integers | empty | Sheet ids a script revision no longer has (kept, not deleted). |
| `days_shot` | array of [day shot](#day-shot) | empty | Each shoot day as it was shot, written back by a production report (decisions.md, entry 224). One per day break. |
| `scenes_shot` | array of [scene shot](#scene-shot) | empty | What was shot of each scene on each day (entry 224). One per scene and day. |

#### Location

| Field | Type | Default | Notes |
|---|---|---|---|
| `name` | string | | Matched to a sheet's `location` without regard to case. Unique the same way. Written trimmed; a name with spaces around it is refused. |
| `latitude_microdeg` | integer | | Millionths of a degree, north positive, -90000000 to 90000000. |
| `longitude_microdeg` | integer | | Millionths of a degree, east positive, -180000000 to 180000000. |
| `utc_offset_minutes` | integer | 0 | Minutes ahead of UTC, -840 to 840: the standard offset when `daylight_saving` names a rule, else the offset on the shoot dates. |
| `daylight_saving` | string | none | When the clocks are an hour ahead of the standard offset (decisions.md, entry 193): `us_canada` (from the second Sunday of March to the first Sunday of November), `eu` (from the last Sunday of March to the last Sunday of October), `australia` (Australia's south-east, from the first Sunday of October to the first Sunday of April; Queensland, the Northern Territory and Western Australia have no daylight saving, so no rule) or `new_zealand` (from the last Sunday of September to the first Sunday of April; decision 220). Left out for none; `none` written, another spelling or another rule is refused. |

#### Day shot

| Field | Type | Default | Notes |
|---|---|---|---|
| `day` | integer | | The day break strip that closes the day, as in `call_times`. |
| `date` | date | none | The date it was actually shot, which may differ from the planned date. |
| `call` | integer | none | The actual call, minutes after midnight (0 to 1439). |
| `wrap` | integer | none | The actual wrap, minutes after the call day's midnight (1 to 2880; past midnight is past 1440), after the call when both are given. |

An entry with none of `date`, `call` and `wrap` is refused: leave it out.

#### Scene shot

| Field | Type | Default | Notes |
|---|---|---|---|
| `sheet` | integer | | The sheet. |
| `day` | integer | | The day break of the day it was shot on, which need not be the day it is planned on. |
| `eighths` | integer | | How much of the scene was shot that day, in eighths of a page (0 to 7992). |
| `complete` | boolean | `false` | `true` on the day that finished the scene, at most once per sheet. `false` written is refused (left out instead). |

An entry with no eighths that doesn't finish its scene is refused: leave it out. A scene
shot over two days has two entries, the second with `complete`. A file without these keys
reads and writes back byte for byte; a reader that doesn't know them (Method Schedule 0.1.0)
refuses the file, as it refuses any unknown key.

#### Lock

| Field | Type | Default | Notes |
|---|---|---|---|
| `name` | string | | Unique without regard to case. |
| `board` | integer | | The board locked. |
| `made` | date | none | When the lock was made. |
| `scenes` | array of `{sheet, sub_board, day, date?, cast}` | empty | Every scheduled scene when locked: its sheet, sub-board, shoot day number, date (left out when the sub-board had none) and cast element ids. |

## Category

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `name` | string | | Unique, ignoring case. |
| `auto_board_id` | boolean | `false` | New elements get the next whole-number board ID: one more than the highest in the category, or 1. |
| `exclude_from_board` | boolean | `false` | The setting a new element of the category gets (decision 120); an element's own key decides. |
| `defaults` | [rules](#rules) | all default | Rules given to elements created in this category. Changing them does not change existing elements. |
| `cast` | boolean | `false` | The built-in cast category (the board shows its board IDs; script import fills it). Written only when the cast category has been renamed away from `Cast`; when no category carries it, the category named `Cast` (ignoring case) is the cast. At most one. |
| `daily_quantity` | boolean | `false` | "Use in daily quantity": its elements' [quantities](#quantity) on each sheet add up in the daily category totals. Off, each element counts once a day. |
| `properties` | array of strings | empty | Custom element properties: the names of the fields its elements may fill in, in order. Unique ignoring case, trimmed, not empty. |
| `multi_line` | array of strings | empty | The properties whose values take several lines (decision 119), by their names exactly as in `properties` and in that order; the others take one line. Files without it read every property as one line and write back byte for byte. |

## Element

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `category` | integer | | A category id. |
| `name` | string | | Unique within its category, ignoring case. |
| `board_id` | string | none | Cast numbers and the like; letters allowed. Unique within the category. |
| `board_id_locked` | boolean | `false` | |
| `exclude_from_board` | boolean | its category's `exclude_from_board` | Leaves this element off the board: no ID on strips or the one-line, never a conflict. Its own setting decides (decision 120). Left out when it matches its category's setting, so a missing key means the category's: a file from before, where a category's switch left all its elements off, reads the same and writes back byte for byte; an element back on the board in such a category writes `false`. |
| `rules` | [rules](#rules) | all default | |
| `notes` | string | `""` | |
| `links` | array of integers | empty | Element ids. |
| `red_flags` | array of [red flag](#red-flag) | empty | In the order added. |
| `properties` | array of [property value](#property-value) | empty | Values of its category's custom properties, in the category's order. A property with no value is left out. |
| `images` | array of strings | empty | Image files, as paths: relative to the folder the schedule file is in (with `/` between folders), or absolute. The images themselves are never inside the schedule file; see [Images](#images). Each once, none empty. |

### Property value

| Field | Type | Default | Notes |
|---|---|---|---|
| `name` | string | | A property of the element's category, spelled as the category spells it. |
| `value` | string | | Not empty. |

### Images

An element's images stay files of their own beside the schedule, so the schedule file stays
small and diffs stay readable. The app stores a path relative to the schedule file's folder
when the image is inside that folder (or a folder under it), and the absolute path
otherwise; keep a production's images in a folder next to its schedule (for example
`images/`) and the paths survive moving or sharing the whole folder. A path whose file is
missing is kept and shown as missing; nothing is ever copied into or out of the schedule
file.

### Rules

Day out of days switches.

| Field | Type | Default | Notes |
|---|---|---|---|
| `holds` | boolean | `false` | |
| `drops` | boolean | `false` | Only acts when `holds` is on too. |
| `min_drop_gap` | integer | `1` | The shortest gap, in calendar days, that can become a drop. |

### Red flag

Dates the element can't, or would rather not, work (decisions.md, entry 33). A warning
only. The dates are given one of three ways: `from` and `to` together, or `not_before`
alone, or `not_after` alone.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `type` | integer | | A [red flag type](#red-flag-type) id. |
| `from` | date | none | With `to`: the first date ruled out. |
| `to` | date | none | With `from`: the last date ruled out, inclusive; not before `from`. |
| `not_before` | date | none | Alone: every date before this one is ruled out. |
| `not_after` | date | none | Alone: every date after this one is ruled out. |
| `note` | string | `""` | |
| `hard` | boolean | `false` | A hard limit (can't work) rather than a soft one (would rather not). |

## Red flag type

A kind of red flag, from the schedule's own list ("Unavailable", "Second job"). Global
across boards.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `name` | string | | Unique, ignoring case. |
| `color` | string | none | `#rrggbb`: the color the type's marks are drawn in (decisions.md, entry 77). None: the usual red. |
| `builtin` | string | none | A standard type's event kind key (`travel`, `fitting`, `rehearsal`, `unavailable`, `camera_test` or a post-production kind; never `other`; decisions.md, entry 113). A standard type can be renamed and recolored but not deleted. New schedules and templates start with one per kind; a file without the key reads and writes back byte for byte. An unknown key is refused. |

## Strip colors

The strip colors the user set (decisions.md, entry 60), for every board. Colors are
`#rrggbb` strings (either case on reading, lowercase on writing).

| Field | Type | Default | Notes |
|---|---|---|---|
| `rules` | array of [color rule](#color-rule) | empty | In order: the first rule that matches a scene strip gives its color. A strip no rule matches keeps the usual colors. |
| `scene_text` | string | none | Text color on scene strips. None: black or white, whichever reads on the strip. |
| `banner_text` | string | none | Text color on banners. |
| `break_text` | string | none | Text color on day breaks. |

### Color rule

| Field | Type | Default | Notes |
|---|---|---|---|
| `int_ext` | string | `""` | The sheet's I/E, compared trimmed and ignoring case. Empty matches any. |
| `time_of_day` | string | `""` | The sheet's time of day, the same way. |
| `color` | string | | `#rrggbb`. |
| `text` | string | none | The pair's own text color, `#rrggbb` (decisions.md, entry 108). When set it wins over `scene_text` on the strips this rule colors. Left out when none, so older files read and write back byte for byte. |

## Sheet

A breakdown sheet. Every field except `id` is optional.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `number` | string | `""` | The sheet number as typed (`12`, `12A`): an index of the user's own, apart from the scene numbers. Sheets can be stepped through by it. |
| `scenes` | string | `""` | Scene numbers as written: `12`, `12A`, `12-14`. |
| `int_ext` | string | `""` | Free text, usually `INT`, `EXT`, `INT/EXT`. |
| `set` | string | `""` | |
| `time_of_day` | string | `""` | |
| `sunrise` | integer | none | Minutes after midnight, 0 to 1439. |
| `sunset` | integer | none | Minutes after midnight, 0 to 1439. |
| `eighths` | integer | `0` | Page length in eighths: `11` is 1 3/8 pages. A Fountain import works it out at 56 lines a page unless the user set another count (decision 195); the count is the user's setting, not in the file. |
| `shoot_minutes` | integer | none | Estimated shoot time. |
| `synopsis` | string | `""` | |
| `location` | string | `""` | |
| `script_pages` | string | `""` | |
| `script_day` | string | `""` | |
| `screen_seconds` | integer | none | Screen time. |
| `unit` | string | `""` | |
| `sequence` | string | `""` | |
| `episode` | string | `""` | |
| `comments` | string | `""` | |
| `elements` | array of integers | empty | Element ids on this sheet, in the order added. |
| `quantities` | array of [quantity](#quantity) | empty | How many of some of its elements the sheet needs, in the order of `elements`. An element with none is one, counted daily. |
| `images` | array of strings | empty | Image paths for the scene, as an element's [images](#images) are. |

### Quantity

| Field | Type | Default | Notes |
|---|---|---|---|
| `element` | integer | | An element on this sheet, at most one quantity each. |
| `count` | integer | | At least 1. |
| `daily` | boolean | `true` | Adds to the day's total for its category when the category uses daily quantities. Off for a group already counted on another sheet that day; the total is then shown as partial. Written only when `false`. |

## Board

One arrangement of every sheet.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `name` | string | | |
| `strip_layout` | array of [strip column](#strip-layout) | the default layout | What the board's scene strips show, left to right. Left out when it is the default. |
| `strip_size` | string | `normal` | The strip layout's text size step, `small` or `large` (decisions.md, entry 87). Left out at `normal`. |
| `strip_red_flags` | boolean | `true` | `false`: the board's own strip layout hides the red flag marks on strips (the strip designer's switch, decisions.md, entry 136). Left out while they show, so older files read and write back byte for byte. |
| `strip_conflicts` | boolean | `true` | `false`: it hides the conflict marks on strips (entry 136). Left out while they show. |
| `day_look` | [day look](#day-look) | totals only | What its day breaks show. Left out at the default. |
| `uses` | object | none | The named layouts it picked instead of its own: `strips`, `day_breaks`, `shoot_day`, each a layout id, each left out when it uses its own (decisions.md, entry 78). |
| `sub_boards` | array of [sub-board](#sub-board) | | At least one. |
| `boneyard` | array of [strip](#strip) | empty | Unscheduled sheet strips, and day breaks parked there (no day, no date; decisions.md, entry 57). Never a banner. |

### Day look

What a board's day breaks show besides their totals (decisions.md, entry 67), on the
board as `day_look`, left out when nothing is on. Each field is left out at its default.

| Field | Type | Default | Notes |
|---|---|---|---|
| `shoot_day_strip` | boolean | `false` | A shoot day strip heading each day, in place of the day heading: the day's number of the sub-board's days and its full date. |
| `screen_time` | boolean | `false` | The day's screen time. |
| `sunrise` | boolean | `false` | The earliest sunrise of the day's sheets: typed, else worked out at a located sheet's place for the day's date (decisions.md, entry 167). |
| `sunset` | boolean | `false` | The latest sunset of the day's sheets, the same way. |
| `quantity` | boolean | `false` | The day's element quantities: each category that uses daily quantities with its total for the day (decisions.md, entry 87). |
| `text` | string | `""` | One line, at most 200 characters, on every day break of the board. |

### Strip layout

Which fields a board's scene strips show, in order, where and how (decisions.md, entries
39, 78 and 87). The board, the one-line report and the printed board use it. A strip has
up to four lines: the first line's fields are its columns, one each, at their widths; a
field on a later line sits under them, after the one before it on that line, covering
`span` of those columns (a merged cell). At least one field on the first line; each field
at most once, except `spacer` and `text`; no later line covers more columns than the
first line has. A board without `strip_layout` shows the default: `scene`, `int_ext`,
`set`, `time_of_day`, `pages`, `cast`, each at its default width. Every field but `field`
is left out at its default.

| Field | Type | Default | Notes |
|---|---|---|---|
| `field` | string | | `scene`, `int_ext`, `set`, `time_of_day`, `pages`, `cast` (the cast's board IDs, or names where an element has none), `location`, `script_day`, `unit`, `sequence`, `episode`, `shoot_time`, `synopsis`, `cast_count` (how many cast members), `element_count` (how many elements, from the categories on the board, or with `category` that category's name and count, as `Props (2)`; decisions.md, entries 66 and 147), `elements` (the elements other than the cast, from the categories on the board, by board ID or name; entry 102), `category_elements` (the scene's elements of the column's `category`, by name or, with `board_ids`, by board ID, as `LAMP, ROPE`; empty with no category; entry 194), `spacer` (an empty cell) or `text` (the column's own `text`, the same on every strip; entry 87). |
| `width` | string | the field's | `narrow`, `medium`, `wide` or `widest`. Left out at the field's default: `medium` for `time_of_day` and `text`, `wide` for `set`, `cast`, `location`, `elements` and `category_elements`, `widest` for `synopsis`, `narrow` for the rest. |
| `second_line` | boolean | `false` | On the strip's second line (decisions.md, entry 78). |
| `line` | integer | none | On the third or fourth line: `3` or `4` (entry 87). The second line is always written as `second_line`, so files from before read and write back unchanged; `line` with any other value, or with `second_line`, is refused. |
| `span` | integer | `1` | How many of the first line's columns a field on a later line covers. On the first line it is always 1. |
| `align` | string | the field's | `left`, `center` or `right`. Left out for the field's usual way (numbers to the right, the rest to the left). |
| `bold` | boolean | `false` | |
| `italic` | boolean | `false` | |
| `text` | string | `""` | A `text` field's text: one line, at most 100 characters. Refused on any other field. |
| `rule_after` | boolean | `false` | A line down the field's right edge, between it and the next column (decisions.md, entry 185). |
| `rule_under` | boolean | `false` | A rule along the field's foot, under its line (entry 185). |
| `category` | integer | none | The category id an `element_count` counts (decisions.md, entry 147): the strip shows the category's name and how many of its elements the scene has, leaving out elements left off the board, as `Cast Members (4)`, and `Props (0)` with none. Left out, the count counts every element, a bare number, empty at none. On a `category_elements`, the category whose elements it lists (entry 194); left out, the cell is empty. Refused on any other field, and for a category the schedule lacks; deleting the category leaves it out. In a [layout file](#layout-files) it is `category_name` instead. |
| `board_ids` | boolean | `false` | A `category_elements` lists board IDs (the name where an element has none) rather than names (decisions.md, entry 194). Refused on any other field. |

A reader refuses an unknown field, width, alignment or size.

### Named layout

One layout of the schedule's library (decisions.md, entry 78), which boards pick by id
(`uses`) and the one-line report too (`one_line_layout`).

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `name` | string | | Unique among the layouts, ignoring case. |
| `kind` | string | | `strips`, `day_breaks` or `shoot_day`. |
| `columns` | array of strip layout columns | | A `strips` layout's columns, as in [strip layout](#strip-layout), always written. |
| `size` | string | `normal` | A `strips` layout's text size step, `small` or `large` (entry 87). Left out at `normal`. |
| `red_flags` | boolean | `true` | `false`: a `strips` layout hides the red flag marks on strips (entry 136). Left out while they show. |
| `conflicts` | boolean | `true` | `false`: a `strips` layout hides the conflict marks on strips (entry 136). Left out while they show. |
| `look` | [day look](#day-look) | nothing on | A `day_breaks` layout's switches and text, or a `shoot_day` layout's `text`. Left out when nothing is on and there is no text. |

A `strips` layout with a `look`, or a `day_breaks` or `shoot_day` layout with `columns`,
`size`, `red_flags` or `conflicts`, is refused.

### Layout files

A layout file (`.mtlayouts`) holds named layouts to load into any schedule:
`{"format": "method-schedule-layouts", "version": 0, "layouts": [...]}`, each layout as
above without its `id` (a layout file with an `id` is refused). An element count names
its category by name, `category_name` (a string, as the category is named in the
schedule written from), never by `category` id, which is one schedule's (decisions.md,
entry 147), and so does a `category_elements` (entry 194); `category_name` is refused in a
schedule, on any other field, and blank or with spaces around it. Loading gives each a new
id and, when its name is taken, a number after it; the category is the loading schedule's
of that name, ignoring case, and where it has none the count counts every element and the
list is empty. A reader refuses what it
refuses in a schedule.

### Sub-board

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | integer | | |
| `name` | string | | |
| `start` | date | none | The first possible shoot day. Needed for dates and the day out of days. |
| `calendar` | [calendar](#calendar) | the default | |
| `strips` | array of [strip](#strip) | empty | In board order. Each day break closes a shoot day; strips after the last one are not scheduled. |

### Calendar

| Field | Type | Default | Notes |
|---|---|---|---|
| `weekly_off` | array of strings | `["sat", "sun"]` | Weekdays that are never shoot days: `mon` to `sun`, written Monday first. |
| `work_days` | array of dates | empty | Dates on a weekly day off marked back to working. Written sorted. |
| `off_days` | array of dates | empty | Single days off. Written sorted. |
| `holidays` | array of dates | empty | Written sorted. |
| `moves` | array of dates | empty | Company travel days. Written sorted. |
| `events` | array of [event](#event) | empty | |
| `prep_start` | date | none | The first day of prep, for the budget exchange ([EXCHANGE.md](EXCHANGE.md)). |
| `wrap_end` | date | none | The last day of wrap, for the budget exchange; not before `prep_start`. |

Shoot day *n* falls on the *n*th date, counting from `start`, that is not a weekly day
off (unless listed in `work_days`), a single day off, a holiday or a company travel day.
The app keeps each date in at most one of `work_days`, `off_days`, `holidays` and `moves`;
a reader that finds one in several reads it as a company travel day, then a holiday, then
a day off.

### Event

| Field | Type | Default | Notes |
|---|---|---|---|
| `kind` | string | | `travel`, `fitting`, `rehearsal`, `unavailable`, `camera_test`, a post-production kind (`post_production`, `visual_effects`, `dubbing`, `sync_sound`, `music_recording`, `picture_editing`, `sound_editing`, `sound_mixing`, `titles_graphics`, `trailer`, `color_correction`; decisions.md, entries 109 and 163) or `other`. The first three count in the day out of days; the others show on the calendar only and are never conflicts. |
| `label` | string | `""` | For `other` only: what kind of event it is. |
| `name` | string | `""` | |
| `from` | date | | |
| `to` | date | | Inclusive; not before `from`. |
| `elements` | array of integers | empty | Element ids, any number. |
| `color` | string | none | One of ten: `red`, `orange`, `yellow`, `green`, `teal`, `blue`, `indigo`, `purple`, `pink`, `brown` (decisions.md, entry 109). Left out when none, so older files read and write back byte for byte. |

### Strip

A tagged object: `kind` comes first and decides the other fields.

| `kind` | Fields | Notes |
|---|---|---|
| `sheet` | `id`, `sheet` | `sheet` is a sheet id. |
| `banner` | `id`, `text`, `minutes` (default `0`), `bold` (default `false`), `color` (default none), `tall` (default `false`), `size` (default `normal`), `align` (default the left), `font` (default the board's) | `minutes` counts toward the day's time. `color` is one of `yellow`, `orange`, `red`, `pink`, `purple`, `blue`, `teal`, `green`, `gray`, or any color as `#rrggbb` in lowercase (an uppercase one is refused, so a file writes back byte for byte; decisions.md, entry 154) (none: the board's banner color); `tall` draws it two lines high (decisions.md, entry 65). Also `size` (`small` or `large`) and `align` (`left`, `center` or `right`), each left out at its default (entry 87), and `font` (`serif`, `sans` or `mono`), left out for the board's own (entry 187). |
| `day_break` | `id` | Closes a shoot day. |

## Consistency

A reader refuses a file where:

- an id appears twice, is 0, or is not below `next_id`; `next_id` is past 1,000,000,000;
- a sheet has more than 999 pages (`eighths` past 7992), a shoot time past 24:00
  (`shoot_minutes` past 1440), a screen time past 24 hours (`screen_seconds` past 86400),
  or a quantity past 99,999; a banner's `minutes` is past 1440 (decisions.md, entry 89);
- an element points at a missing category, or a sheet, event or link at a missing element;
- a red flag points at a missing red flag type, gives its dates other than one of the three
  ways, or ends before it starts;
- a red flag type has an empty name, or two share a name (ignoring case), or two are built
  in as one event kind;
- a sheet or an event lists one element twice, or an event ends before it starts or covers
  more than 366 days (no edit makes a longer one; decisions.md, entry 133);
- a category has an empty name, or two categories share a name (ignoring case);
- two elements in a category share a name (ignoring case) or a board ID;
- a category has an empty property name, one longer than 60 characters or on more than
  one line, or two of one name (ignoring case); an element has a value for a property its
  category lacks, an empty value, two values for one property, or its values out of the
  category's property order; an element links to itself or to one element twice; an image path is empty or
  given twice;
- a sheet has a quantity for an element not on it, two for one element, or a count of 0,
  or an image path that is empty or given twice;
- `sheet_fields` names a field twice (an unknown field is refused as a bad value);
- an event other than `other` has a `label`;
- there is no board;
- a board has no sub-board, or doesn't hold every sheet across its sub-boards and
  Boneyard, or has a strip for a missing sheet; a sheet may have several strips on a
  board's sub-boards (the parts of a scene spanning days, decisions.md, entry 76), but a
  sheet in the Boneyard has no other strip on that board;
- a strip layout (a board's or a named one) has no field on its first line, shows a field
  other than `spacer` or `text` twice, has a field on a fifth line or later, a field on
  the first line covering more than one column or one covering none, a later line
  covering more columns than the first line has, or custom text on a field other than
  `text`, longer than 100 characters or on more than one line (decisions.md, entries 78
  and 87);
- a day look's text is longer than 200 characters or on more than one line;
- a Boneyard holds a banner (it holds sheet strips and parked day breaks only);
- a report has an empty or repeated name, a date range that ends before it starts, a
  missing or repeated category or element, or a column twice;
- two day out of days codes are the same, or one is empty, too long or holds anything
  but letters, digits and `/`;
- a calendar's `wrap_end` is before its `prep_start`;
- a calendar's `weekly_off` lists all seven days, or a day twice, or is not Monday first;
  or its `work_days`, `off_days`, `holidays` or `moves` are out of date order, list a
  date twice, or share a date with another of the four (a date is exactly one kind);
- a named layout has an empty, long or repeated name or an unsound body, or a board or
  the one-line report picks a layout that isn't there or isn't of that type;
- the plan names a location twice (ignoring case), one with an empty name or a
  latitude, longitude or UTC offset out of range, a call time past 23:59 or two for one
  day, a long day limit or turnaround out of range, two turnarounds for one category, a
  lock with an empty, repeated, multi-line or longer than 200 characters name, a strip
  pinned twice or a pin on a day break or banner, or a sheet marked omitted twice;
- a file marked `"template": true` holds sheets, strips, events, red flags, locks, call
  times, pinned strips or omitted scenes;
- the strip colors have more than 200 rules, two rules for the same I/E and time of day
  (ignoring case), or a value longer than 200 characters; a color that isn't `#rrggbb`
  is refused as a bad value.

## Example

A schedule with one scene on one shoot day, TOVE unavailable for two days the week
after, and strips that show the scene, the set (at its widest) and the cast IDs:

```json
{
  "format": "method-schedule",
  "version": 0,
  "schedule": {
    "title": "Harbor Light",
    "next_id": 11,
    "categories": [
      {
        "id": 1,
        "name": "Cast"
      }
    ],
    "red_flag_types": [
      {
        "id": 9,
        "name": "Unavailable"
      }
    ],
    "elements": [
      {
        "id": 6,
        "category": 1,
        "name": "TOVE",
        "board_id": "1",
        "rules": {
          "holds": true
        },
        "red_flags": [
          {
            "id": 10,
            "type": 9,
            "from": "2027-03-08",
            "to": "2027-03-09",
            "note": "Another shoot",
            "hard": true
          }
        ]
      }
    ],
    "sheets": [
      {
        "id": 4,
        "scenes": "1",
        "int_ext": "INT",
        "set": "NET LOFT",
        "time_of_day": "NIGHT",
        "eighths": 2,
        "elements": [
          6
        ]
      }
    ],
    "boards": [
      {
        "id": 2,
        "name": "Board 1",
        "strip_layout": [
          {
            "field": "scene"
          },
          {
            "field": "set",
            "width": "widest"
          },
          {
            "field": "cast"
          }
        ],
        "sub_boards": [
          {
            "id": 3,
            "name": "Main",
            "start": "2027-03-01",
            "strips": [
              {
                "kind": "sheet",
                "id": 5,
                "sheet": 4
              },
              {
                "kind": "day_break",
                "id": 7
              }
            ]
          }
        ]
      }
    ]
  }
}
```
