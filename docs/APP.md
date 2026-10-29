# The desktop app (`crates/method-schedule-app`)

Method Schedule's editor is a local desktop app built with [Tauri 2](https://tauri.app): the Rust
engine runs natively inside the app, and the interface is plain HTML, CSS and JavaScript
drawn in the operating system's own web view. Schedules are JSON files on your disk
([FORMAT.md](FORMAT.md)). There is no server and no account, and no network access but the update check (decision
213), which can be turned off. The shape
follows Method Budget's app (`../method-budget/crates/method-app`, same owner and license),
scaled down to a first slice.

This is phase 2: one schedule per window, any of its boards and sub-boards, and the Board,
Breakdown, Elements, Day out of days, Calendar, Reports, Conflicts and Red flags tabs, with drag and
drop, banners, sorting, a strip layout, cut, copy and paste, and duplicating and merging strips on
the board, red flags on elements, Find and Replace, printing, PDF and spreadsheet
export, Explain this count, What Changed and Merge Changes From. "Not yet" at the end lists what comes later.

## Run and build

You need Rust (the toolchain is pinned in `rust-toolchain.toml`). There is no JavaScript
toolchain: no npm, no bundler, no packages.

```bash
cargo run -p method-schedule-app            # run a debug build
cargo test -p method-schedule-app           # session, board, view, report, export, explain, merge, red flag, menu and capability tests, no window
python3 scripts/make-icons.py     # redraw the app icons in crates/method-schedule-app/icons from the mark
```

The files in `crates/method-schedule-app/ui/` are embedded into the binary at compile time, so after
editing them, run `cargo run -p method-schedule-app` again to see the change. Release
bundles are made with the Tauri CLI; see [Updates and releases](#updates-and-releases).

**Linux** needs the WebKitGTK development packages to build (CI installs them):

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libssl-dev build-essential pkg-config libgtk-3-dev
```

macOS and Windows use the system web view (WebKit and WebView2) and need nothing extra.

**The page without the app.** `crates/method-schedule-app/harness/` loads the real page with a fake
`window.__TAURI__` (a small in-memory board with the view's shape, its categories,
elements and calendar) so drawing, keys and the arguments the page sends can be checked in
any browser:

```bash
python3 scripts/serve-harness.py   # caching off, so JS changes always load
# then open http://127.0.0.1:8765/harness/index.html
```

The harness is not part of the app: `tauri.conf.json` embeds only `ui/`. Its fake mimics
the engine's rules roughly; the real rules are tested in Rust. It starts with one recovered
schedule to offer, logs every call in `window.__calls`, and sends events the way Rust does
with `window.__emit(name, payload)` to every window and `window.__emit(name, payload, 'main')`
to the harness page's own window (`window.__menu(id)` for a menu item); the page listens on
its own window label, so an event sent to another label is not heard. Setting
`window.__bigDood = { rows: 300, days: 140 }` before opening the Day out of days tab gives
a large synthetic grid for checking scrolling and the rows drawn (on the Reports tab too,
for paging the day out of days); `window.__bigReport = 40` adds 40 synthetic shoot days to
the one-line and shooting schedules. `print_report`, `export_report` and `export_budget`
record what they were asked in `window.__printed` (with the page count and footers in
`#print-root` at that moment) and `window.__exported`; `window.__cancel = true` makes the
next one act as if its dialog was cancelled (the What Changed and Merge Open dialogs too).
`explain_count` retells the fake grid's row; `what_changed` and the merge commands are
canned, `merge_apply` records what it was asked in `window.__merged`,
`window.__mergeUnclear = true` makes the automatic base fall back to the file on disk and
`window.__mergeUntitled = true` makes the merge refuse as for a schedule never saved.
The fake keeps a report list with its options, production info, code letters, prep and
wrap dates and the user's preferences roughly as Rust does (decision 59); its calendar
lists each shoot day's strips with their drop places, and its day out of days sorts.
`find_text` and `replace_text` mimic the engine's search and its main refusals on the fake
schedule (sheets' header fields, banners, events, categories and elements, in board order),
and `conflicts_view` works the conflicts out roughly from the start dates and day breaks;
give a second sub-board a start date and a scene with KIT to see one. The fake starts with
three red flag types (Travel and Unavailable standard, Second job its own; the other
standard types missing, so Red Flag Types offers to add them) and no flags; its red flag commands check
dates as Rust does and work the clashes out roughly (shoot days from the start date and
day breaks, travel days, fittings and rehearsals), so with Main starting Mon 2027-03-01 a
flag on KIT for 2027-03-02 marks scene 3, that date and KIT's cell.

## Using it

- **File**: New, Open, Open Recent, Save, Save As, Autosave, Revert to Backup and Import
  Script, in the
  File menu and (all but Open Recent) in the top bar. New and Open replace the open
  schedule, asking first about unsaved changes (Save, Don't Save, Cancel) when autosave
  couldn't take care of them. Saving and recovery are described [below](#saving-and-recovery). Import Script reads a Fountain (`.fountain`) or Final
  Draft (`.fdx`) file and adds one strip per scene to the end of the Boneyard, with the
  speaking characters as Cast elements (decisions.md, entry 8). A Fountain script is
  paginated at **Fountain lines per page** from Settings (56 unless changed, 7 lines an
  eighth; entry 195); a Final Draft file keeps its own lengths. An untitled schedule that
  hasn't been touched takes the script's title. Saves suggest the working extension
  `.mtsc`; Open also accepts `.json`.
  **Save as Template…** writes the schedule's setup (categories, elements and their
  settings, red flag types, strip colors and layouts, the report list, production info,
  code letters, header field order, boards and sub-boards with their calendars) without
  sheets, strips, events or red flags, as our own schedule file marked as a template
  (`.mtst`), suggesting the data folder's `Templates/`; the open schedule is not changed.
  **New from Template…** starts a new, untitled schedule from a template, or from any
  schedule file taken as one, in this window as **New** does: the template is picked
  first, then the unsaved-changes question when anything is unsaved (Autosave off, or no
  file yet); with Autosave on and a file, the edits are already in it. **New Window**
  is the way to keep this schedule open beside the new one. Opening a template with Open
  does the same, so a template is never written over by accident (decisions.md, entries
  77, 85 and 166).
  **New Window** (⇧⌘N) opens a window with a new, untitled schedule, and **Open in New
  Window…** opens a schedule in a window of its own, set 26 points below and right of
  the window it opens from (back to the screen's top left when it would run off the
  screen), so it never hides that window exactly; each window is one schedule, with
  its own undo, autosave and recovery copy, and its title says `(edited)` while
  it has unsaved changes. A file already open in a window (from Open, Open Recent, Open in
  New Window or the system) brings that window to the front instead of opening twice;
  Save or Save As into a file another window has open, or a recovered copy of it, is
  refused (`NAME is open in another window.`).
  Close Window (⌘W) and closing a window ask about its unsaved changes (once, however
  often the close button is clicked); Quit asks each window in turn, and Don't Save
  takes effect only once every window has answered, so Cancel keeps every window's
  changes safe. The **Window** menu lists every window (macOS lists them in its own
  Window menu); a menu command acts on the window in front, and Recovered schedules is
  offered in the first window only. Each page hears its own window's events and the
  app-wide ones (the Autosave switch, preferences changed in any window); nothing under
  a schedule's lock waits for the main thread (decisions.md, entries 86 and 88).
- **Edit**: Undo and Redo, up to 200 steps. Every change to the schedule is one step.
  Cut, Copy and Paste act on what has focus: in a text field, the text, as always; on
  the Board tab, the selected strips; on the Breakdown tab, the chosen elements (below).
- **Density**: the page is laid out to show at least as much as the reference app
  (decisions.md, entry 53): 13 px text (11 px at the least), tight toolbars, tab bar,
  board bar, dialogs and side panels, 22 px strip lines. At 1280 x 800 the top bar (with
  the views in it), board bar and footer take 94 px (131 before entry 204, 183 before
  entry 53), 37 px more for the board than before.
- **The look** (decisions.md, entry 204): the brand family's warm paper and ink neutrals
  with the slate blue accent, light and dark (docs/BRAND.md). New, Open, Save, Save As,
  Undo and Redo are icons, each named by its tooltip and its accessible name; the views
  are one segmented control in the top bar. Settings > Look offers two more (entry 208): **Graphite**,
  always dark with colored strip edges, and **Ledger**, always light with full strip
  colors and ruled columns. Paper stays the default. Each strip's first cell (usually the scene
  number) is set apart with a faint tint and a rule, as in the mark (entry 206).
- **The status line** at the left of the footer says what a command did or why it was
  refused. It keeps its one line whether or not it has a message (a long message ends in
  an ellipsis, its whole text on hover), so a message appearing or clearing never moves
  the board under the pointer (decisions.md, entry 48). A message clears after six
  seconds; an error stays until the next message. **Error lines** in the tabs and
  dialogs (a refused date on the Calendar, a field a dialog refuses) do the same: each
  keeps its one line of room while empty, so an error never moves the controls below it
  (decisions.md, entry 107).
- **The board bar** under the tabs picks the **board** shown and the **sub-board** the
  Elements, Day out of days, Calendar and Reports tabs, File > Export, Export for Budget and
  Explain this count use (kept while it is on the board shown, else its first). Choosing a
  board is not an edit: nothing to undo, nothing unsaved (decisions.md, entry 31).
- **Boards and sub-boards** (Board menu, and Boards… in the bar): New Board (one sub-board,
  Main, starting today, every scene in its Boneyard; decision 153), Duplicate Board (a copy of the board shown with its
  sub-boards, calendars, strip order, banners and day breaks, named `<name> copy`, shown
  next), New Sub-board (it starts today, decision 123), and the Boards and Sub-boards dialog: rename in place, Up and Down
  to reorder, Show, Duplicate, and Delete (a second click confirms). Board names are unique
  ignoring case; sub-board names are unique within their board. A board and a sub-board
  each keep at least one. Deleting a board keeps the scenes on the others; deleting a
  sub-board moves its scenes to the end of the Boneyard, and its day breaks, banners and
  calendar go with it. Each sub-board's start date is set on the Board tab. Every change
  is one undo step.
- **Board tab**: the board shown's sub-boards, each as its shoot days. A day shows a header
  (day number and date), its strips (by default scene, I/E, set, time of day, page length
  in eighths, cast board IDs or names; see Strip Layout below), and a dark day-break row with the day's scene count, pages and
  shoot time. Strips after the last day break are listed as not scheduled. The Boneyard
  follows. Strip colors follow the usual convention (until set with Strip Colors, below): interior day white, exterior day
  yellow, interior night blue, exterior night green; banners have their own color, with
  their duration at the right.
  - **Selecting**: click a strip; Shift-click selects the run from the last strip clicked
    (across day breaks, sub-boards and the Boneyard), Command-click (Ctrl-click) adds or
    drops one, Shift+Up and Shift+Down grow the run.
  - **Drag and drop**: drag a strip, or the selection, within a sub-board, across day
    breaks, to another sub-board or into the Boneyard. A line shows where it lands (above
    or below the strip under the pointer; first in a day when over its header; red where
    the drop is refused). Near the top or bottom edge the board scrolls by itself. Escape
    cancels. Strips that weren't next to each other gather at the drop in board order. A
    day break dropped in the Boneyard is **parked** there: drawn faint as "Day break,
    parked", it closes no day, has no number or date and counts nowhere (day out of days,
    reports, conflicts, the budget exchange) until it is moved back (decisions.md, entry
    57). Banners never go in the Boneyard. A drop is one undo step (decisions.md,
    entry 29). Option-Up and Option-Down move the selection as a block.
  - **Context menu**: right-click a strip, on a sub-board or in the Boneyard (Control-click
    on a Mac), or press Shift+F10 or the context menu key with the board focused. A strip
    not yet selected is selected first; a selected one keeps the selection. The menu is
    drawn in the page (no native menu, so no new capability): Edit Sheet (Edit Banner on a
    banner), Cut, Copy, Paste, Delete, Merge Strips (two or more scenes), Duplicate Strip,
    Move To (the other sub-boards and the Boneyard, to the top; the selection goes to the
    strip that followed it, decisions.md, entry 129), Insert Day Break Above or
    Below, Insert Banner Above or Below. Each item runs what the menu bar and the Edit menu
    run; items that can't act are dimmed. Up and Down move, Right opens a submenu, Left
    closes it, Enter runs, Escape closes (decisions.md, entry 53).
  - **Day breaks**: Board > Insert Day Break Below (⌘D, or B on the board) and Above
    (⌥⌘D), also in the context menu, put a day break below the bottommost selected strip
    or above the topmost; with nothing selected, at the end or start of the
    picked sub-board (decisions.md, entry 55).
  - **Banners**: Board > Insert Banner Above (⌘B, the default, as the reference app's New
    Banner) or Below (⇧⌘B) the bottommost selected strip (with several selected, both go
    by the bottommost, as the reference app; with none, the start or end of the sub-board
    shown; decisions.md, entry 143) asks for
    the text and a duration as `h:mm` (24:00 at most, entry 50), which counts toward the
    day's shoot time, and its look: Color (a palette of 49 swatches: the board's banner color,
    nine named colors that go lighter or darker with the theme and forty more by value,
    plus any other color from the system picker; entry 154), Height (one or two lines; the text wraps onto the second), Bold, Text size
    (Small, Normal, Large), Alignment (Left, Center, Right) and Font (the board's, Serif,
    Sans serif, Monospaced: families every system has a face for), saved with the banner
    (decisions.md, entries 65, 87 and 187). Enter or a double-click on a banner edits
    it; editing keeps the look unless it is changed.
  - **Delete or Backspace** deletes the selected strips as one undo step: day breaks
    without asking, banners and scenes after asking. Deleting a scene's strip deletes its
    sheet, so its strip leaves every board, as Delete Sheet on the Breakdown tab does; its
    elements stay in the schedule (decisions.md, entries 30 and 54).
  - **Sort Strips** (Board menu) sorts the list holding the selected strip (a sub-board or
    the Boneyard) by up to four fields (scene, I/E, set, time of day, location, pages,
    script day, unit, sequence, episode), each ascending or descending. Banners go to the
    top, day breaks are removed (break the days again after), scenes with the field empty
    come last, ties keep their order. Scene numbers sort naturally: 9, 10, 10A, 11, A1.
    One undo step (entry 32).
  - **Cut, Copy and Paste** (Edit menu, ⌘X, ⌘C, ⌘V) work on the selected strips: scenes,
    banners and day breaks. Paste puts them below the bottommost selected strip (with
    nothing selected, at the end of the picked sub-board), on this board or another, and
    selects them (the real app leaves a pasted copy unselected and cuts and pastes strips
    from their context menu, not the Edit menu; ours differs on purpose, entry 52). Strips cut on the board they are pasted on **move** there; cut strips
    stay where they are, dimmed with a dashed edge, until the paste. Anything else pastes
    **copies**: a scene becomes a new sheet with the copied sheet's fields and elements
    (its strip at the paste, and at the end of every other board's Boneyard), banners and
    day breaks new strips. So a cut pasted on another board leaves the originals where
    they were, and the status line says so. A cut is used up by its paste; a copy can be
    pasted again. Banners never go in the Boneyard: such a paste is refused with a sentence
    (day breaks park there). Each paste is one undo step; Cut and Copy change nothing. The
    clipboard is the app's own: it holds the strips as they were when copied, and empties
    when another schedule opens. Plain text (a line per strip, its fields separated by
    tabs) goes to the system clipboard as well (decisions.md, entry 38).
    **Between windows** the clipboard is one: strips cut or copied in one window paste
    in another window's schedule, always as copies (a cut leaves the originals in their
    own schedule, and the status line says so). Each scene's elements are matched by
    category and name ignoring case, and added when that schedule lacks them (a
    category too, with its settings); its quantities come along; header values take the
    spelling that schedule already has (entry 155). The paste is one undo step there.
    Elements cut or copied stay in their own window: copying them empties the other
    windows' clipboards (decisions.md, entry 183).
  - **A scene spanning days**: Board > Continue Scene on Next Day (or Continue on Next
    Day in the context menu) adds a part of the selected scene first in the day after the
    next day break (adding a day break after the scene when it has none). Each part is a
    strip of the same sheet, marked `2/3` after its scene number; the scene's pages and
    times count on its first part only, its elements work on every part's day (day out of
    days, conflicts, red flags). Delete on a part removes that part alone; a scene with
    parts stays on its sub-boards (the Boneyard refuses it). One undo step each
    (decisions.md, entry 76).
  - **Day Breaks…** (Board menu) chooses what the board's day breaks show besides their
    day, scenes, pages and shoot time: the day's screen time, the earliest sunrise and the
    latest sunset of its scenes (typed on the sheet, else worked out for the day's date at
    a location that Plan > Locations places on the map; entry 167), the day's element quantities (each category that uses
    daily quantities with its total, `Background 45`, an asterisk when partial; entry
    87), and a line of text; and a **shoot day strip** heading each
    day in place of the day heading (`Shoot day 3 of 12 · Wednesday, March 3, 2027`; the
    first at the top of the sub-board, each next just under the End of day line before it;
    entry 148): None, the day and date, or
    a named shoot day layout with its own text. **Day break layout** picks the board's own
    settings or a named day break layout; Save as Day Break Layout and Save as Shoot Day
    Layout keep them as named layouts the board uses. Saved with the board in the file,
    one undo step each; the printed board follows (decisions.md, entries 67 and 78).
  - **Show on Board…** (Board menu) shows or hides the day headings, the day breaks, the
    totals on the day breaks and the banners. Hidden lines can't be selected, and a drop
    lands by the strips shown around it; the schedule, the reports and the day counts
    don't change. Remembered in this browser (localStorage), not in the schedule; the
    printed board follows it (decisions.md, entry 64).
  - **Strip Colors…** (Board menu) sets a color for each I/E and time of day the scenes
    use (one row each, found on the board, plus Add for values no scene uses yet or an
    empty field for any value), and a text color for scene strips, banners and day breaks
    (Automatic: black or white, whichever reads on the strip). Each pair can also have its
    own text color (Text, on its row), which wins over the scene text color on its strips
    (decisions.md, entry 108). The first color that
    matches a strip wins; a value with no color keeps the usual one. Saved in the schedule
    file (FORMAT.md, "Strip colors"), shown on every board; one undo step (decisions.md,
    entry 60).
  - **Strip Layout…** (Board menu) chooses which fields the board's scene strips show, in
    what order and how wide: scene, I/E, set, time of day, pages, cast IDs, location,
    script day, unit, sequence, episode, shoot time, synopsis, and the resource counts
    Cast count and Elements (empty when none; decisions.md, entry 66), Category elements
    (one category's elements on the scene, `Props` heading `LAMP, ROPE`; entry 194: a part
    under the list picks the category, found by its name, and Board IDs lists board IDs
    in place of names), spacers and custom
    text (Add Spacer, Add Text: as many as wanted), each Narrow, Medium, Wide or Widest
    (Wide and Widest share the room left over), on Line 1 to Line 4. Show, the width, the
    line, the columns it covers (lines 2 to 4), Usual, Left, Center or Right, B and I,
    the line styles `|` (a line after the field) and `_` (a rule under it; decisions.md,
    entry 185), a custom text's text, and Up and Down for each field; Text size (Small, Normal,
    Large) for the layout; Default Layout puts the usual six back. The first line's
    fields are the columns; a field on a later line sits under them and covers as many
    as chosen (a merged cell). A strip is one row of the board as many board lines high
    as its layout has lines (long text ends in an ellipsis), so the windowed board keeps
    its arithmetic: each list keeps a running sum of its lines' heights and finds the
    line at a scroll position or under the pointer by a binary search (board.js,
    `lineTops` and `lineAt`). The board, the printed board and the one-line reports draw
    a strip's cells with one renderer (strips.js, `stripGrid`, `stripCell`, `sceneRows`):
    on paper a later line is a table row of its own under the strip, a merged cell a
    cell spanning columns (decisions.md, entry 87). **Layout** at the top picks what the board uses: its own layout
    (saved with the board, FORMAT.md "Strip layout") or a **named layout** from the
    schedule's library, whose fields the list then shows; Save edits the one picked (a
    named layout changes on every board that uses it), **Save as New Layout** keeps the
    fields as a named layout the board uses, and Delete Layout removes a named one (its
    boards go back to their own). **One-line report uses** picks the board's layout or a
    named strip layout for the one-line report, on screen and printed (a category's
    elements lists there as on the board, and in its spreadsheet). Each change is one
    undo step (decisions.md, entries 39 and 78).
  - **Layouts…** (Board menu) lists the named layouts (strips, day breaks, shoot day):
    rename in place, Up and Down, Delete (a second click confirms), how many boards use
    each, **Save to File…** (the checked ones, or all) as a layout file (`.mtlayouts`,
    our own JSON, FORMAT.md "Layout files") and **Load from File…**, which adds a layout
    file's layouts (a name already used gets a number). The file dialogs are Rust's.
  - **Duplicate Strip** (Board menu) copies each selected scene's sheet, every field and
    element, to a new sheet whose strip goes right after the original (on every board
    where that is on a sub-board; in the Boneyard, at its end) and selects the copies.
    **Merge Strips…** (two or more scenes selected) shows what the merge does before
    doing it: the fields the merged sheet gets (scenes joined, pages, shoot and screen
    times summed, synopses and comments joined), the elements it gains, and every value
    left out because it differs from what the merged sheet keeps. The **first selected**
    scene (the click before the Command-clicks or Shift-clicks) keeps its sheet, its
    fields and its place on every board; the others' scenes and elements join in the
    order they were selected, the survivor's first, and their strips leave every board.
    Banners and day breaks stay where they are. A Shift-click run counts from the anchor
    toward the row clicked. Each is one undo step (decisions.md, entries 35, 37 and 46).
  Each sub-board has a start date (dates appear on the days once it is set) and an
  automatic day break run. Every date field in the app (the start date, an event's From
  and To, a red flag's dates) is a text field: type `3/1/2027`, `03/01/2027` or
  `2027-03-01` and leave the field (or press Enter); it shows the date back as
  `2027-03-01`, and anything else is refused with a sentence saying how to type it
  (decisions.md, entry 49). For the day break run, pick what limits a day under **Break
  by** (Pages, Shoot time or Screen time), type the limit (pages as `5` or `4 4/8`, shoot
  time as `h:mm`, screen time as `m:ss`) and choose Break: every day break on the
  sub-board is replaced, and a day closes as soon as it reaches or passes the limit
  (banners' durations count toward shoot time; decisions.md, entries 12 and 56). The
  status line says how many shoot days that made. **Remove Breaks** beside it (and
  Board > Remove All Day Breaks, on the selection's sub-board) removes every day break on
  the sub-board as one undo step, leaving its strips in order, unscheduled.
  Past 120 lines (day headers, strips, day breaks, captions) the board draws only the lines
  in view plus 15 either side, each line a fixed 22 px (26 px before entry 53), with spacers sized through a
  constructed stylesheet as in the day out of days grid. Selection and the board keys work
  on the data: moving to a row that isn't drawn scrolls to it and draws it. The board keeps
  its scroll place while another tab is open (decisions.md, entry 24).
- **Breakdown tab**: one breakdown sheet at a time. Double-click a scene on the Board (or
  select it and press Enter) to open its sheet. Choosing the Breakdown tab (its tab, the
  View menu or a Breakdown menu command) with a scene selected on the Board shows that
  scene's sheet too. It keeps the sheet shown last with a banner or day break selected,
  and when it is chosen from a tab other than the Board with the board's selection
  unchanged since it last showed a sheet (decisions.md, entry 51). The header line says which sheet it is in
  the board shown's order (each sub-board's strips in turn, then the Boneyard), its scene
  and where its strip is on that board (`Main, day 3, Wed 2027-03-03`, `Main, not scheduled` or `Boneyard`).
  That place is a link: a click shows the Board tab with the sheet's strip selected and
  scrolled into view (decisions.md, entry 58).
  Previous and Next (⌘[ and ⌘], Ctrl+[ and Ctrl+] elsewhere) step through the sheets in
  board order, or by sheet number or scene number as picked in **by** beside them (natural
  order, sheets without one last; remembered in this browser); ⇧⌘[ and ⇧⌘] go to the
  first and last sheet. Breakdown > **Go to Scene…** (⌘J) and **Go to Sheet Number…**
  open the sheet whose scenes or sheet number are exactly what is typed (decisions.md,
  entries 79 and 81).
  - **Split screen** (View > Breakdown, Board, Calendar or Images Beside Board, ⇧⌘2,
    ⇧⌘1 and ⇧⌘5; decisions.md, entry 80): on the Board tab a pane beside the board shows
    the selected scene's sheet (it follows the board's selection and edits live, its keys,
    Cut, Copy and Paste working as on the Breakdown tab), another board or one sub-board
    (picked in the pane; click, drag, the start date and the day break run work there,
    each one undo step), the calendar, or the selected scene's images and its elements'.
    The bar between the panes drags (Left and Right on it too); which pane and how wide are
    remembered in this browser. **Single View** in the pane's head (⇧⌘0) closes it. Another
    tab shows full width; back on the Board tab the pane comes back. View > **Images
    Beside Breakdown** puts the images pane beside the Breakdown tab instead: the sheet
    shown's images and its elements', following the sheet as you move between sheets;
    Single View (or the item again) closes it, and the Breakdown tab remembers it apart
    from the board's choice (decisions.md, entry 110).
  - **Header fields**, in three tight rows with each label beside its field, as the
    reference app lays the sheet out (decisions.md, entry 53): Sheet (the sheet number),
    Scenes, I/E, Set, D/N (time of day) and Shoot time; Synopsis, Location, Pages, Script
    pages and Script day; Screen time, Unit, Sequence, Episode, Comments, Sunrise and
    Sunset. Breakdown > **Breakdown Designer…** puts them in another order (Up, Down,
    Default), hides fields and sets widths (entry 101), and shows the shown fields in
    their rows: a field dragged to another place or row, or to New row, gives rows of
    the user's own lengths (entry 186). All kept in the schedule; without rows set they
    hold six, five and the rest.
    **Decimal time** in the head shows the shoot time as decimal hours (`1.50`); `1.5` and
    `1:30` can both be typed whichever is shown. **Images** under the fields: Add Image…
    (a Rust Open dialog, the path kept as an element's is), thumbnails, × to take one off
    (decisions.md, entries 77 and 81). Synopsis and
    Comments are one line high and grow when dragged. Scenes, I/E, set, time of day, pages as whole pages plus eighths (eighths of 8 or more roll into
    pages: 0 + 11/8 is 1 3/8), shoot time as `h:mm` (24:00 at most), screen time as `m:ss`, sunrise and
    sunset as 24-hour `hh:mm`, synopsis, location, script pages, script day, unit,
    sequence, episode and comments. A field is sent when you leave it (or press Enter in
    a one-line field), and each change is one undo step. Times and pages are read
    strictly, integers only: `90` is not a shoot time, `1/2` is not a number of eighths. A
    refused value stays in its field, marked, with a plain sentence above the form, until
    you fix it or move to another sheet. Text is trimmed and capped at 10,000 characters.
    The one-value fields (set, location, I/E, time of day, unit, sequence, episode and
    script day) list the values other sheets use as you type (those starting with the
    text first, at most eight; Up and Down pick, Enter or a click takes one, Escape closes
    the list), and a value that matches one ignoring case takes its spelling: `room 2`
    where a sheet has `ROOM 2` is `ROOM 2` (decisions.md, entry 155).
  - **Elements**: every category is a small box in a grid (as many columns as fit: five
    at 1280 px), in category order, empty ones too, each with a colored left edge, its
    name and how many of its elements are on the sheet, its elements one per line, and an
    Add field. Each element has a remove button (×); removing takes it off this sheet
    only. A box's Add field matches as Quick add does, within that category: Up and Down
    pick a match and Enter adds it, Enter with nothing picked adds the typed name to that
    category. **Hide empty** (beside Quick add) leaves out the boxes with nothing on the
    sheet; it is remembered in this browser (localStorage), not in the schedule. With 23
    categories, all 23 boxes and all 18 header fields fit a 1280 x 800 window without
    scrolling (before: one category box whole in view, empty ones never shown).
    Adding an element adds the elements it links to as well (Quick add, a box's Add
    field, Paste). Each element's quantity button (`×1`, faint until hovered) sets how
    many the sheet needs and whether that counts in the day's total for its category
    (`×35*` marks one left out of the total; decisions.md, entry 63). **New Event…** in the
    header line opens the event editor with the sheet's elements picked, from the date it
    shoots on (or the sub-board's start), for the sub-board the other tabs show.
  - **Cut, Copy and Paste of elements**: click an element's name to choose it
    (Command-click or Ctrl-click adds or drops one, Shift-click a run; Escape clears),
    then Edit > Cut or Copy. Step to another sheet and Paste (with focus anywhere on the
    tab but a field): copied elements are added to that sheet, each once; cut elements
    move there, leaving the sheet they were cut from, and stay marked until then. Each
    paste is one undo step. The element names go to the system clipboard as text.
  - **Quick add**: type a name and the elements that start with it (then those that
    contain it) appear below, leaving out those already on the sheet. Up and Down pick
    one and Enter adds it. Enter with nothing picked adds the element of that name,
    ignoring case, from any category (the first in category order when two have it;
    decisions.md, entry 157); a name no element has asks for its category from a list
    with the one shown next to the field (New in) chosen, so Return adds it there and
    Escape goes back with the text kept (entry 156). `Category;Name` creates without
    asking (a new category is added if you type one). Focus stays in the field, ready for
    the next.
  - **New Sheet** adds an empty sheet right after the current one when it is on a
    sub-board, otherwise at the end of the Boneyard (and at the end of every other
    board's Boneyard), then puts the cursor in Scenes. **Duplicate Sheet** copies the
    sheet, every field and element, as Board > Duplicate Strip does, and shows the copy.
    **Delete Sheet** asks first, then removes the sheet and its strip from every board;
    its elements stay in the schedule, and Undo brings it back. The three are buttons on
    the tab and items in the Breakdown menu, which shows the tab first.
- **Elements tab**: the categories down the side (with how many elements each holds), and
  for the one picked, its settings and its elements.
  - **Categories**: add one by name (names are unique, ignoring case), rename it, move it
    up or down among the user categories, and delete it once it is empty. The cast category is
    built in: it stays first and can't be deleted, but it can be renamed (decisions.md,
    entries 19 and 42).
  - **Category settings**: number new elements automatically (one more than the
    highest whole number in the category, or 1), **Use in daily quantity** (quantities on
    sheets add up in the daily category totals), and the defaults new elements get: **Off
    the board** (a new element starts left off the board: no strip's cast column, never a
    conflict, also on the day out of days; decision 120) and the hold and drop rules. They
    reach new elements only; **Apply to Existing Elements** (it asks first) gives every
    element in the category these defaults, as one undo step. **Custom properties**: the category's fields
    (Height, Agent): type a name in New property and press Enter; rename one in place,
    move it earlier with ‹, delete it with × (it asks; every value of it goes too).
  - **The list**: board ID (with "locked"), name, sheets (occurrences), pages, work days,
    start and finish, events (on the sub-board) and conflicts (on this sub-board, in red),
    in board ID order then by name; click a column heading to sort by it, again to
    reverse, a third time for the usual order (this session only). Work days and dates
    come from the picked sub-board and need its start date.
  - **Header fields**: Set, Location, Unit, Episode, I/E, Time of day, Script day and
    Sequence under the categories (decision 118), each with how many values the sheets
    give it. Picking one lists its values with sheets, pages, work
    days, start and finish, worked out from the sheets and never edited here (inventory
    section 4; decisions.md, entry 62). The Day out of days tab offers them too.
  - **One element**: pick its name to edit it below the list. Name (unique in the
    category, ignoring case), board ID (letters and digits, at most 12, unique in the
    category; empty clears it), Lock board ID, Leave off the board (this element; its own setting decides, decisions 105 and 120), holds, drops and the minimum drop gap
    (1 to 999 days). Move to another category (its name and its board ID must be free
    there, or the move is refused; it keeps its ID, or no ID even where the category numbers automatically, decision 140; its property values are lost). Delete Element asks first; a linked element
    asks to unlink it as well. Deleting takes it off every sheet and event. Below the
    rules: a field per custom property of its category, **Notes** (several lines; they
    commit when left), **Links to** with **Linked Elements…** (tick the elements that come
    with it whenever it is added to a sheet; optionally add them to every sheet that has
    it now), **Images** with **Add Image…** (a Rust Open dialog; the schedule keeps the
    path, relative to the schedule's folder when the image is inside it, and shows a
    thumbnail Rust reads; × takes an image off, the file stays), **New Event…** (the event
    editor with this element picked, saved to the sub-board's calendar), and **Days and
    dates on** the sub-board: every date of its day out of days row with the shoot day,
    the code and what it means.
  - **Merge**: tick two or more elements, choose Merge, and pick the one to keep. Every
    sheet and event that listed any of them lists it once.
  - **Renumber**: from a start number, in the order shown (by the sorted column when one
    is sorted). Locked elements keep their IDs
    and their numbers are skipped; IDs with letters stay as they are; every other element,
    with or without an ID, gets the next free number.
  - **Red flags** (below the element's fields): the dates it can't work, or would rather
    not, as a table of type, dates, hard or soft and note, with **Add Red Flag…**, Edit and
    Delete (it asks). The dialog takes a type, the dates (from a date to a date, not
    before a date, or not after one; typed as `3/1/2027` or `2027-03-01`; an empty To takes the
    From date), a note and Hard (on by default; off means soft). A range that ends before
    it starts is refused in the dialog. Several flags per element and per day are fine.
  - **Red Flag Types…** (here, on the Red flags tab and in the Edit menu): add a type,
    rename it in place, Up and Down to reorder, Delete (a second click confirms). Names
    are unique ignoring case; each shows how many flags use it, and a type in use can't
    be deleted (it says so). Each type has a color (a color field; Red for none): its
    marks everywhere are drawn in it (decisions.md, entry 84). New schedules start with
    the standard types, one per event kind but other (travel, fitting, rehearsal,
    unavailable, camera test and the post-production kinds), marked Standard: renamed
    and recolored, never deleted. An older schedule keeps its own list; **Add the
    Standard Types** adds the missing ones in one step (decisions.md, entry 113).
  Every change is one undo step; a refused one says why above the tab and changes nothing.
- **Red flag marks** (decisions.md, entries 33 and 36): a scheduled strip whose shoot day a
  red flag rules out shows a red ⚑ before its scene number and a red right edge; the
  mark's accessible name and title say each clash (`Red flag: ADA, Unavailable,
  2027-03-02 to 2027-03-04, hard`). The Calendar marks such dates the same way (with the
  clashes in the date's title and in the side panel for the picked date), and the Day out
  of days outlines the clashing cells in red, the clash in the cell's title. A clash is
  the element working that date on the sub-board, or at a travel day, fitting or
  rehearsal there; holds, days off and other events never clash, and every element
  counts, also one in an excluded category. Hard and soft flags are warnings alike. The
  mark takes the color of the first clashing flag's type. The Breakdown tab lists the
  clashes on the date its sheet shoots, and the Elements table counts each element's in
  a Red flags column (decisions.md, entry 84).
- **Conflict marks** (decisions.md, entry 83): a strip involved in a conflict shows a ⚠
  before its first column, white on a small red badge so it reads on any strip color (a
  red flag's ⚑ sits on a white one; entry 149), and a red left edge, its title saying each conflict (`KIT on
  2027-03-02 also on Unit B day 3`, `KIT at Fitting on a work day, 2027-03-02`); the
  Calendar marks the date on each sub-board involved, and the Breakdown tab lists the
  sheet's conflicts under its head.
- **Day out of days tab**: the grid for the picked sub-board and one category
  (the cast category unless another is picked), or one of the header fields under Header
  fields in the picker (a row per set, location, unit, episode, I/E, time of day, script
  day or sequence value, worked out from the
  sheets; Explain This Count works on elements only): one row per element in the engine's order
  (board ID in natural order, then name), one column per date from the first to the last
  date any row covers, with the engine's codes (`SW`, `W`, `WF`, `SWF`, `SWD`, `PW`, `PWF`,
  `H`, `FTH`, `TR`, `/`, `M` and so on, decisions.md entries 10 and 11). An event on a
  work day shows the event's code and then the whole work code (`FTW`, `FTPW`, `TRSW`,
  `FTWF`) with a dashed red outline: it is a conflict (entries 43 and 47). Each column shows its date, weekday and shoot
  day number; days off are shaded, holidays and company travel days too, and a bar marks
  each element's span from start to finish. The summary stays at the right: work, hold,
  start, finish and total (work plus hold). The ID and name columns, the summary and the
  header stay put while the dates scroll, and past 80 rows only the rows in view are drawn
  (decisions.md, entry 20). It needs a start date and says so until there is one. The
  columns are narrow, as the reference app's (entry 53): 32 px a date, 36 px the ID, 120 px
  the name, 20 px rows; a code too long for its cell (five letters or more) ends in an
  ellipsis, and every cell's title gives its date and whole code. At 1280 x 800, 27 date
  columns and 26 rows are in view at once (23 and 20 before: 702 cells against 460).
  - **Explain this count**: click a row's Work, Hold or Total (or select the row and press
    Enter, or Edit > Explain This Count, ⌘I) and a panel beside the grid says why the
    counts are what they are (`method_schedule_core::explain`, docs/MERGE.md): the totals, what work
    is made of, the rules and the drop, every gap between work or event days with what
    became of it (Dropped, Held, Counts nowhere) and why, and every date of the row with
    its code and reason. A click on a date there scrolls the grid to that column and marks
    it. Click an ID or name to select a row; Up and Down move the selection while the grid
    has focus (past 80 rows a row out of view is scrolled to and drawn). The panel is
    worked out again after every change while it is open; Close or Escape shuts it
    (decisions.md, entry 26).
  - **Sort, switches and views** (decision 59): a click on the ID, Element, Work, Hold,
    Start or Finish heading sorts by it (again: the other way; Total is no sort key and
    does not); **Sort…** takes up to four keys (board ID, name, start, finish, work days, hold days), each ascending or
    descending. Rows missing a value go last, ties keep their order, and the sort lasts
    while the tab and the sub-board are shown: leaving the tab or picking another
    sub-board clears it, and Sort… then opens with no keys (entry 145; it is never saved). Each row's **H** and **D** boxes are
    the element's hold and drop switches, one undo step each. **Without holds** shows the
    grid as if no element held (a session view). **Vertical** turns the grid on its side:
    a row per date, a column per element, the counts at the foot. **Saturdays** and
    **Sundays** each show or hide that day's columns (rows when vertical), remembered for
    this viewer on this computer (decisions 121 and 158); counts never change. The DOOD
    report follows each (decision 141). **Codes…** changes the
    code letters the schedule keeps (1 to 3 letters, digits or `/` each, no two the same);
    the grid, the legend, the reports and the spreadsheets show them.
- **Calendar tab**: one month of the picked sub-board, six weeks, Sunday to Saturday (or
  Monday to Sunday, as Week starts in Settings has it). Each
  date shows its shoot day number (`Day 3`) or what it is (Off, Holiday, Travel day, or
  Working on a weekly day off marked back to working) and its events. Previous Month, Next
  Month and First Shoot Day move around; in the month the arrow keys move the picked date
  (a day, or a week up and down) and Page Up and Page Down change the month. Today and a
  Go To date field move there too (decision 59). Holidays carry a star and company
  travel days a plane beside the date. **Month** and **Weeks** pick the view: Weeks is one
  continuous list of every week from the schedule's first date (its start, its first
  event) to its last (its last shoot day, its last event), and the month last shown,
  scrolled with the page; Page Up and Page Down move the picked date four weeks there. The
  month view is the default; the pick is remembered in this browser (decisions.md, entry
  111).
  - **Actions** (the button beside the picked date's title, or a right-click on a date):
    Working Day, Day Off, Company Travel Day, Holiday, Add Event…, Add Red Flag… (pick
    the element, then the red flag dialog opens on that date) and Set as Start Date (the
    sub-board's start, as on the Board tab). Each is one undo step.
  - **On each date** (the View switches): shoot day and day off labels, page counts,
    estimated time, scenes per day, events, strips, banners and red flags, each on or
    off, with All and None. Kept in the schedule (FORMAT.md, `calendar_hidden`), like the
    calendar strip fields; one undo step each. A shoot day lists its banners above its
    strips.
  - **Strips on the month**: each shoot day lists its scene strips in board order (the
    fields picked with **Strip Fields…** and kept in the schedule, scene, set, elements
    and cast IDs by default, decisions.md, entry 102), its pages and
    estimated length. Drag a strip onto another shoot day to move it to the end of that
    day (`move_strips`, one undo step); the drag is the page's own, with pointer events as
    the board's, since the Tauri window keeps HTML drag and drop (decisions.md, entry 160). **Show** filters the strips with two small
    drop-downs of check boxes, each with All: the categories in use, and the elements
    under their category headings (those of the checked categories, or every one). With
    elements checked, only strips holding one of them show; with only categories checked,
    strips holding any element of them. Day headers and events always show; the filter is
    for this session (entry 146). A click outside or Escape closes an open list, and that
    Escape does nothing else (entry 168).
  - **Preferences**: **Week starts** (Sunday or Monday) and **Dates** (2027-03-01,
    3/1/2027 or Mon 1 Mar 2027) are the user's, kept in the app's `settings.json`, not in
    the schedule, as is **Fountain lines per page** (40 to 70, default 56; Import Script
    and Import Script Revision use it; decisions.md, entry 195); the calendar, the day out
    of days and the reports follow them, and date
    fields show ISO or US back. A change reaches every open window. Each setting in the
    file is read on its own: one that is wrong falls back to its default and leaves the
    others (Autosave stays off when it was off).
  - **Prep starts** and **Wrap ends**: the sub-board's prep and wrap dates, kept in the
    schedule for the budget exchange file.
  - **Print…** and **Save as PDF…** print the Calendar report of the Reports tab.
  - **The picked date**: make it Working, Day off, Holiday or Company travel. Holidays and
    company travel days take no shoot day number, so later shoot days move to the next
    working dates, on the Board and in the day out of days.
  - **Days off each week**: Monday to Sunday switches (Saturday and Sunday by default). At
    least one weekday stays working.
  - **Events**: New Event (from the picked date) asks for the kind (travel, fitting,
    rehearsal, unavailable, camera test, a post-production kind, or other with what kind
    it is), an optional name, from and to dates (at most 366 days), a color from ten (or
    none), and any number of elements, picked from the schedule by category with a
    filter. The post-production kinds (post-production, visual effects, dubbing, sync
    sound, music recording, picture editing, sound editing, sound mixing, titles and
    graphics, trailer, color correction) show on the calendar only: never in the day out of days, never a
    conflict. An event's color edges its line in each date and its badge in the lists
    (decisions.md, entry 109). Edit and Delete (it asks first) are on each event, in the side panel
    for the picked date and in the list of every event. Travel, fittings and rehearsals
    count in the day out of days; unavailable and camera tests neither show there nor
    count (behavior note 17, A8). New Event is also on an element (Elements tab) and on a
    sheet (Breakdown tab). Drag an event's line in a date to another date to move it
    there, keeping its length, one undo step (decisions.md, entry 83).
  Every change is one undo step.
- **Reports tab**: the schedule's report list for the picked sub-board, each a preview of
  exactly the pages that print (Letter paper; portrait or landscape under Paper, landscape
  by default for the day out of days, the calendar and the vertical one-line). Every page
  has a header (the schedule's title, the report, the date printed) and a footer (board
  and sub-board, Page X of Y). Our own plain layouts (decisions.md, entries 23 and 59).
  - **The list**: every kind of report at least once, in the schedule's order. Up and
    Down reorder it, **Copy** adds a named copy with the same options (`<name> copy`),
    Delete removes a copy (the last of a kind stays). Saved in the schedule; each change
    one undo step.
  - **Options** of the picked report, each change one undo step: Name; a From and To
    date range; Paper; Text (small, normal, large); Header (full: title, production info,
    board, sub-board, start and range; compact: one line; title only); the switches that
    apply to its kind (Synopsis, Banners, Cast names rather than IDs, Page per day, Strip
    colors, Red flag marks, Without holds, Weekdays only); and the pickers: Categories,
    Elements and Columns (the report designer, decisions.md, entry 90: the scene fields
    shown, in order, each with its line, the first line's columns it covers (a merged
    cell), alignment, bold, italic and its own text size; Default follows the board's
    strip layout for the one-line reports).
  - **Production Info…**: the title, company, director and producer the full header
    prints, kept in the schedule.
  - **One-line schedule**: each shoot day's heading (day and date), a line per scene with
    the board's strip layout as its columns (by default scene, I/E, set, time of day,
    pages, cast board IDs), the synopsis under the set unless the layout shows it as a
    column of its own, and banners, then its end line (scenes, pages, shoot time). A day stays on one page when
    it fits, so page breaks fall between days.
  - **Shooting schedule**: each day starts a new page: every scene's header line,
    synopsis, location and its elements by category, then the day's end line.
  - **Day out of days**: the grid for each category with anyone working or on an event
    (the engine's codes, days off shaded, a line under each work span), in whole weeks
    from the week's first day (Weeks start on, so a range from Monday March 1 starts on
    Sunday February 28): as many weeks as fit in 28 dates to a landscape page and 18 to a
    portrait one. The DOOD tab's **Saturdays** and **Sundays** switches decide each day: off,
    it is left out (with both off, Friday is followed by Monday and a week is five
    columns); on, it prints shaded as a day off (decisions 141 and 158). Then the summary (work, hold, start,
    finish, total) by category. Short grids share a page.
  - **Elements**: every element by category: board ID, name, sheets, pages, work days,
    start and finish.
  - **Daily category totals**: a row per shoot day and a column per category, each count
    how many different elements of it the day uses. Categories no day uses are left out,
    and the page says how many.
  - **Breakdown by strip**: a block per sheet in board order with its day and date: its
    fields, synopsis, location and elements by category (a page each with Page per day).
  - **Breakdown by day**: each shoot day's scenes (by default scene, I/E, set, time of
    day and pages, as the breakdown by strip; decision 161), then each category's elements
    merged over the day, each once with how many of the day's scenes list it.
  - **Element detail**: each element with its work dates, then every scene it is in with
    day and date.
  - **Calendar**: a month a page, the user's week, each date with its shoot day, scene
    numbers, pages, estimated length, events and red flags.
  - **Vertical one-line** (decisions.md, entry 115): the stripboard on its side, a column
    per scene with its text turned (scene, I/E, set, time of day, pages) and a dark
    column after each day with its pages and time; the left of every page holds the
    producer, director, assistant director and script date (those filled in) over a
    fixed row per cast member in board ID order, one left off the board too (decision
    142). No cover page; 30 columns to a
    landscape page.
  - **Multi-unit one-line**: every dated sub-board of the board merged by date, each
    unit's day under its name; the element filter keeps the days with a scene holding one.
  - **Conflicts** and **Red flags**: the Conflicts and Red flags tabs as printable tables
    (the red flag report: every flag, then the clashes, filtered by category, element
    and date range; decisions.md, entry 114).
  A report with nothing to show (no day breaks, no start date for the day out of days)
  prints one page saying why.
- **Print… (⌘P)** opens the system print sheet with the orientation already set; **Save
  as PDF…** asks where to save and writes the PDF there with no print sheet. On the Board
  tab both print the board (below); elsewhere they print the Reports tab's report (from
  the File menu they show the tab first), laid out again from the schedule as it is.
- **Print Board… and Save Board as PDF…** (Board menu, or File > Print and Save as PDF on
  the Board tab) ask whether to print every strip or only the selected ones, whether to
  add the Boneyard, and the paper's way (portrait or landscape), then print the board
  shown through the same paging and print path as the reports: each sub-board with its
  days (heading, strips in the strips' colors and the user's strip colors, the board's
  strip layout as columns, the day break with its scenes, pages and time), the strips
  after the last day break, then the Boneyard; the header and footer of every page as the
  reports have them (decisions.md, entry 61). A strip line on paper is as dense as on
  the screen: 8.5pt text in a 14.4pt line, as the board's 13 px text sits in its 22 px
  line, a two-line banner two lines (entry 188). On Windows and Linux both use the system print dialog for
  now, whose own PDF choice saves a file.
- **PDF properties** (macOS): a saved PDF's title is the job's name, its subject what it
  shows (the report, board and sub-board, date range), its author the production company
  from the production info, and its creator Method Schedule (decisions.md, entry 96). The
  system dialog's PDF on Windows and Linux carries none of ours yet.
- **Export… (⌘E)** asks for a report (the day out of days summary, the one-line
  schedule, the element list, the day out of days grid, the element detail, the
  breakdown by strip or by shoot day, the whole board or the daily category totals) and
  a format (Excel workbook or CSV), then where to save
  it, suggesting the schedule's folder and `<name> - One-Line.xlsx` and so on. The tables
  are `method_schedule_format::export`'s, the same as `method-schedule dood|oneline|elements --xlsx|--csv`
  print; the CSV leaves out the page-length text column (it repeats the eighths) and
  starts with a UTF-8 byte order mark so Excel reads accented letters. Totals in a
  workbook are working formulas with their values cached (decisions.md, entry 95); a CSV
  has the values.
- **Export for Budget…** writes the budget exchange file ([EXCHANGE.md](EXCHANGE.md)) for
  the picked sub-board, the same file `method-schedule budget` writes (version 1): today's
  local date as `generated`, the hash of the schedule's canonical text as `source_hash`
  and the hash of the days it reports as `days_hash`. It is made
  from the schedule as it is in the app, saved or not, so the hash matches the file on
  disk only once it is saved. It needs a start date and says so.
  Exports never replace the open schedule's file, and every file (PDFs too) is written
  atomically.
- **Find and Replace** (Edit > Find…, ⌘F; Edit > Replace…, ⇧⌘F) opens a panel at the
  right that stays open across the tabs. Type to search (it searches as you type); Match
  case; Whole word (the match doesn't run on into letters, digits or `_`); **Look in**:
  everything, every sheet field, one header field (Scenes, I/E, Set, Time of day,
  Synopsis, Location, Script pages, Script day, Unit, Sequence, Episode, Comments),
  element names, category names, banner text or event names; **Where**: the whole
  schedule, or the sub-board picked in the board bar (its sheets, banners and events, and
  the elements on them with their categories). Results come in board order (each board's
  sub-boards' strips, each sub-board's events after its strips, scenes only in a Boneyard,
  then categories each followed by its elements), each with its place (`Scene 12, Set`,
  `Cast element`, `Banner on Main`, `Event on Unit B, 2027-03-04`) and what the field holds
  around the match; at most 1,000 are listed and the count says how many match. A click
  on a result goes there: a sheet field opens the Breakdown tab on that sheet with the
  field focused and marked, an element or category opens the Elements tab on it, a banner
  is selected on the Board tab (showing its board first), an event opens the Calendar on
  its first date. Enter and Shift+Enter in the search box, the Next and Previous buttons,
  or Edit > Find Next (⌘G) and Find Previous (⇧⌘G) anywhere step through the results.
  While the panel is open, every strip on the board whose scene or banner is in the
  results is outlined, on the board beside the board too (split screen: its scenes, and
  its own board's banners), and **Select on Board** selects them all on the Board tab
  (decisions.md, entries 130 and 184).
  **Replace…** (or ⇧⌘F) adds a Replace with box and a checkbox on every result, all
  checked, with All results above; **Replace** changes the checked results only, as one
  undo step, and says how many places changed. Empty replaces with nothing (the box's
  tooltip says so; its hint is just Replace with, entry 169). Refused, with
  nothing changed and a plain sentence in the panel: two elements of one category or two
  categories with the same name ignoring case, an empty name, a line break in a one-line
  field (all but Synopsis and Comments), and results that no longer match. Names are trimmed,
  and so are the one-value header fields (set, location, I/E, time of day, unit,
  sequence, episode, script day): one replaced with nothing is left empty, so its value
  is unlinked from the sheet and leaves the lists once no sheet has it (entry 189). The panel searches again after every change, Escape or
  Close shuts it (decisions.md, entries 28 and 34).
  **Advanced…** adds rows of field (every text header field, pages, shoot time, screen
  time), comparison (contains, does not contain, is, is not, starts with, is empty, is
  not empty, greater than, less than) and value, joined by **All rows** (AND) or **Any
  row** (OR): only the sheets that meet them are searched and replaced, and with nothing
  typed in the box each such sheet is listed (decisions.md, entries 77 and 82). Each row
  takes two lines in the panel, the field first, then the comparison and the value
  (entry 165).
- **Conflicts tab** (View > Conflicts, ⌘7): one element on the same date in two or more
  sub-boards of the board shown, each with its name and category, the date, and every
  sub-board's shoot day and scenes; then, under **Events on work days**, each travel day,
  fitting or rehearsal on a date the element works on the same sub-board (decisions.md,
  entry 43). Only work days count (not holds), strips after the last day break aren't
  scheduled, and elements left off the board are left out. Apart,
  under **Informational, not conflicts**: the event clashes (an element with two or more
  events on one date, or an event in one sub-board on a date it works in another) and the
  sub-boards not compared because they have no dates. A click on a scene selects its strip on the Board tab, on a date or
  an event shows that date on that sub-board's Calendar, on an undated sub-board puts the
  cursor in its start date. The tab shows how many conflicts the board has in a red badge,
  kept in step after every change, and the list is worked out again after every change
  while it is open (decisions.md, entries 28 and 34).
- **Red flags tab** (View > Red Flags, ⌘8), in three parts (decisions.md, entry 114). At
  the top, adding flags to several elements and dates at once: Elements (a category, then
  a check box per element, All and None, a count), Dates (picked on a small month
  calendar with a count, each run of following dates one flag: a click picks a date
  alone, Command-click or Ctrl-click adds one or removes a picked one, Shift-click adds
  every date from the last date picked to this one and keeps the rest, entry 144; or
  From and To, Not before or Not after typed) and Flag (type, note, Hard, **Add Red Flags**: one undo
  step; at most 10,000 flags in one go). Then **Every red flag**: a table of start, end (Open for an open end), category,
  element, type, hard and note, with a check box per row and one for all shown, an
  Element filter, and **Delete Checked** (a second click confirms; one undo step).
  **Export…** writes the table as a workbook or CSV, **Print…** and **Save as PDF…**
  print the Red flags report. Last, every red flag clash on the sub-board picked in
  the board bar, in date order (then the element's place in the schedule, then the
  flag's place on it): the date, the element and its category, the type, hard or soft,
  the flag's dates and note, and the shoot day with its scenes or the event. A click on a
  scene selects its strip on the Board tab, on the date or an event shows that date on
  the Calendar, on the element opens it on the Elements tab. Without a start date it
  says only events were checked. The tab's red badge counts the picked sub-board's
  clashes, follows the sub-board picker and every change, and the list is worked out
  again after every change while it is open (decisions.md, entry 36).
- **What Changed…** (File menu) asks for another schedule file in an Open dialog and lists
  every change from it to the schedule in the window; **What Changed Since Saved** does
  the same against this schedule's own file as it is on disk (with Autosave on that is a
  few seconds old). The changes are the engine's sentences (`method_schedule_core::diff`, the same
  as `method-schedule diff`), grouped under Schedule, Categories, Elements, Sheets, Boards, Calendars
  and Strips, or "No changes.". Read-only.
- **Merge Changes From…** (File menu) asks for another copy of this schedule (theirs) and
  merges what changed in it into the schedule in the window (yours), against a base: the
  version both started from (`method_schedule_core::merge`, docs/MERGE.md). The dialog shows:
  - **The base**, found automatically: the newest of this file's rolling backups made
    before theirs was last changed (every backup predates yours, which was saved over
    it), with a sentence saying so. When that can't be told (no backups yet, every backup
    newer than theirs) it is the file as it is on disk now, and a highlighted sentence
    says why and what that means: anything saved into the file after the copy was made
    reads as undone in theirs. Any backup (newest first) or the file on disk can be picked
    instead; changing the base starts the picks again.
  - **Conflicts**, each with Yours, Theirs and Was, and Keep yours (the default) or Keep
    theirs. Each choice works the merge out again.
  - **What will change here**: the changes the merge makes to the schedule in the window,
    grouped like What Changed; how many of theirs got new ids; and anything the merge
    kept yours so the schedule hangs together.
  **Merge** applies it as one undo step, worked out on the schedule as it is then; the
  status line says how the conflicts were settled. The result always passes the
  schedule's check. A schedule never saved can't merge (it has no base), and the file
  can't merge from itself (decisions.md, entry 26).
- **Merge Conflicts to Review…** (File menu) lists every conflict a merge settled, from
  this dialog or from `method-schedule merge` and the git merge driver, kept with the file
  until marked done (decisions.md, entry 238): what it was about (a link to the sheet,
  element or category when it is still there), Yours, Theirs and Was, which side was kept
  and when. **Done** takes one off the list and **All Done** every one, each a change Undo
  takes back. To use the other value, go to it and change it there.

### Saving and recovery

Method Budget's design (`../method-budget`, docs/APP.md "Saving and recovery"), scaled
down to one schedule per window. The plain-file parts are `src/safety.rs` and the core of
`src/safety_app.rs`, tested with temporary folders.

**Autosave** is on unless you turn it off: the Autosave switch at the right of the footer, or File >
Autosave (one setting for the app, `settings.json` in its data folder). With it on, a
schedule that has a file is saved into that file 3 seconds after you stop changing it (at
most 30 seconds after the first unsaved change while you keep going), right away when the
window loses focus, and before New, Open, closing the window or quitting. The edited mark
clears after each autosave; Undo and Redo keep working across autosaves. A schedule never
saved (Untitled) can't be saved into a file: it gets a recovery copy instead, and the
first Save asks where. If an autosave into the file fails (the folder is gone, the disk is
full), the app says so once and keeps a recovery copy.

**A full disk, a locked file** (decisions.md, entry 200). A save that runs out of room
part way leaves the file as it was and no temporary file behind, and says `Couldn't save
NAME: the disk is full. Nothing was written, so the file is as it was. Free some space, or
use Save As to save somewhere else.` A file that is read-only, or locked in the Finder,
is never written over (renaming over it would work on Unix, but the lock is the user's
say), and neither is a file in a folder that can't be written to: `Couldn't save NAME:
the file or its folder is locked or read-only. Nothing was written. Use Save As to keep
your changes in another file.` Autosave says the same once and keeps a recovery copy.
The same refusal covers every document write (exports and templates too).

**Autosave off** leaves the file alone until you save, and keeps a recovery copy of the
unsaved changes on the same timing.

**Changes made outside the app.** Autosave uses the same outside-change guard as Save: the
file is read and hashed before every write into it, and if another program changed it
(a sync service, another computer, another app), nothing is written, autosave into it
pauses and the unsaved work goes into a recovery copy. A file that was moved or deleted is
written again. The window also checks its file when it gets focus, when autosave finds
the change, and before Save (size and modification time first, the content hash when
either differs; decisions.md, entry 179), and asks what to do:

- **Only the file changed** (nothing unsaved here): **Load Theirs** (the default) opens
  the version on disk in the window, or **Keep Mine** saves the window's version over it.
- **Both changed**: **Merge** (the default) brings the changes made outside into yours
  with Merge Changes From's dialog, against the version this window last read from or
  saved into the file (entry 180); **Keep Mine** saves yours over theirs; **Load Theirs**
  drops your unsaved changes, after a second question.
- A file that no longer reads as a schedule offers only Keep Mine.
- **Later** leaves everything as before: autosave stays paused, a banner says so, and the
  focus check doesn't ask again about the same version (Save does).

Nothing is ever written over the change without one of these. Keep Mine and Merge put the
version on disk into the backups first, whatever the gap since the last one. Load Theirs is
one undo step: Undo brings your version back, unsaved. A choice is made for the version
the dialog showed; if the file changed again meanwhile, it says so and changes nothing.

**Recovery copies** are the safety net for a crash, a forced quit or a power cut. They live
in the data folder's `Recovery/` (0700 folder, 0600 files on macOS and Linux): the schedule
file and a small `.json` beside it (original path, title, time). A copy is deleted when the
schedule is saved, when Undo takes it back to the saved state, when it is replaced by New
or Open, and when you close or quit and choose Don't Save. Quitting without the guard
(logging out, the system quitting the app) keeps the changes: into the file with Autosave
on, else into a recovery copy. Each running copy of the app holds an exclusive lock on a
small `owner-*.lock` file in `Recovery/`, so a second copy never offers the first one's
live copies. A clean exit (Quit, closing the window, logging out) unlocks and removes that
file; a crash leaves it unlocked, and the next start removes it. Only a lock some running
copy holds counts as live: the check takes a shared lock, so two copies looking at a dead
copy's file at the same moment never mistake each other's check for a live owner.

On the next start, if copies were left behind, a **Recovered schedules** dialog lists each
(title, original file name or "Untitled (never saved)", when it was autosaved) with **Open
Recovered** and **Discard** (a second click confirms; it can't be undone). Later keeps them
for the next start. A recovered schedule opens with unsaved changes, pointing at its
original file; autosave won't write into that file, and the first Save into it asks before
replacing it (Save As doesn't ask).

**Backups.** Before a save replaces a schedule file, the version on disk is copied to the
data folder: `Backups/<hash of the file's path>/<name> <date time> UTC.mtsc`, with a
`source.json` naming the file. An explicit Save always makes one (unless the file is
unchanged since the last backup); autosaves make one on the first write in a session and
then at most every 10 minutes. The last 5 per file are kept. They are plain schedule files
you can open.

**File > Revert to Backup…** lists the open file's backups, newest first, each with its
date and how many changes it is from the schedule now. The one picked becomes the
schedule in this window as one undo step (Undo takes it back), and is saved as any change
is; the file as it is now goes into the backups first (decisions.md, entry 181). A
schedule never saved has none.

**File > Open Recent** lists the last 10 schedules opened or saved (newest first, each
once; two files with one name show their folder), kept in `recent.json`. Files that no
longer exist drop off. Clear Menu empties it.

**Opening from the system.** `.mtsc` is registered as the app's document type, and `.mtst`
as its template type (`tauri.conf.json`, `bundle.fileAssociations`; the type identifiers
`com.methodschedule.mtsc` and `com.methodschedule.mtst`, decision 211; a template opened
this way starts a new, untitled schedule). Double-clicking one, Open With, or dropping
one on the Dock icon opens it in place of the front window's schedule, after the
unsaved-changes question (a file already open in a window brings that window to the
front instead, decisions.md, entry 86): on macOS the app receives an open event, on Windows and Linux the path arrives
on the command line (`method-schedule-app film.mtsc` works too). When several files arrive, the
first one opens. There is no single-instance handling yet on Windows and Linux.

**The launcher's switches** (SUITE-STANDARDS.md section 2): `--check-for-updates` does
Help > Check for Updates as the app starts, and `--open-sample` opens the sample (as Help >
Open Sample) when no file is named. Open Recent's list is `recent.json` in the data folder
(`~/Library/Application Support/com.methodschedule.desktop` on macOS,
`%LOCALAPPDATA%\com.methodschedule.desktop` on Windows (the launcher reads `%APPDATA%`, then `%LOCALAPPDATA%`), `~/.local/share/com.methodschedule.desktop`
on Linux), in the suite's shape: `[{"path": "/abs", "opened": "<RFC 3339>"}]`, newest
first, so the launcher can show it. The app's first shape (`{"files": [...]}`) is still
read and is written back in the new one.

**Crash recovery, tested** (decisions.md, entry 200; `src/safety_crash.rs`). Each promise
above has a test against the saving code: a child process running the same code is
killed with `kill -9` mid-edit and the next run offers and reopens its recovery copies
(an untitled schedule and a file with Autosave off), with the file on disk untouched and
the dead run's lock tidied away; a disk that fills part way through a save (a stand-in
writer that fails as a full disk does) leaves the old file whole and the work in a
recovery copy that outlives a crash; a read-only file, a read-only folder and a file
locked in the Finder are refused in plain words with the copy kept; a file changed
outside the app and then a crash keep both versions. What stays a hand check is the real
window: killing the app itself and seeing the Recovered schedules dialog, the refusal
messages on screen, and the outside-change question on focus. `scripts/crash-check.sh`
builds the debug app, opens it on a temporary copy of the sample, kills it with `kill -9`
and locks and changes the file, telling you what to do and look for at each step.

Every write (a save, an autosave, a recovery copy, a backup, the settings and recent
lists) is atomic: a temporary file in the same folder, flushed, renamed into place, and the
folder flushed on Unix.

The data folder is the platform's local app data folder for `com.methodschedule.desktop`:
`~/Library/Application Support/com.methodschedule.desktop` on macOS, `%LOCALAPPDATA%` on Windows,
`~/.local/share` on Linux.

### Keyboard and menus

| Keys (macOS) | Keys (Windows, Linux) | What |
|---|---|---|
| ⌘N | Ctrl+N | New schedule |
| ⌘O | Ctrl+O | Open |
| ⌘S | Ctrl+S | Save |
| ⇧⌘S | Ctrl+Shift+S | Save As |
| ⌘Z | Ctrl+Z | Undo |
| ⌘X ⌘C ⌘V | Ctrl+X Ctrl+C Ctrl+V | Cut, Copy and Paste: text in a field, else the selected strips (Board tab) or the chosen elements (Breakdown tab) |
| ⇧⌘Z | Ctrl+Y or Ctrl+Shift+Z | Redo |
| ⌘Q | Ctrl+Q | Quit (asks about unsaved changes) |
| ↑ ↓ | Up, Down | Select the strip above or below |
| ⇧↑ ⇧↓ | Shift+Up, Shift+Down | Grow or shrink the selection from where it started |
| ⌥↑ ⌥↓ | Alt+Up, Alt+Down | Move the selected strips one place, as a block. At the top or bottom of a sub-board they move to the end of the list above or the start of the list below, so scenes move between the Boneyard and the sub-boards from the keyboard |
| B | B | Insert a day break below the selected strip |
| ⌘D ⌥⌘D | Ctrl+D Ctrl+Alt+D | Insert a day break below or above the selection (Board menu) |
| ⌘, | Ctrl+, | Settings… (week start, date format and Fountain lines per page, for this computer) |
| ⌘W | Ctrl+W | Close Window (asks about unsaved changes) |
| ⌘J | Ctrl+J | Go to Scene… (Breakdown menu) |
| ⌘L, ⌘K | Ctrl+L, Ctrl+K | Linked Elements… and Add Red Flag… for the element picked on the Elements tab (Edit menu) |
| ⇧⌘1 ⇧⌘2 ⇧⌘5, ⇧⌘0 | Ctrl+Shift+1, 2, 5, 0 | Board, Breakdown or Calendar beside the board; Single View (View menu) |
| ⇧⌘[ ⇧⌘] | Ctrl+Shift+[ Ctrl+Shift+] | First and last sheet (Breakdown tab, and the breakdown beside the board) |
| ⌘/ | Ctrl+/ | Help > Keyboard Shortcuts: every command and its keys |
| Esc | Escape | Closes any dialog or sheet, wherever focus is in it (a list or menu open in it closes first; decisions.md, entry 164) |
| ⇧F10 | Shift+F10 or the context menu key | The strips' context menu, on the selection (right-click opens it on a strip) |
| ⌫ or ⌦ | Backspace or Delete | Delete the selected strips (scenes and banners after asking; a scene's sheet leaves every board) |
| ↩ | Enter | Open the selected scene's breakdown sheet, or edit the selected banner |
| ⌘B, ⇧⌘B | Ctrl+B, Ctrl+Shift+B | Insert a banner above or below the selection (Board menu) |
| ⌘[ ⌘] | Ctrl+[ Ctrl+] | Previous or next sheet in board order (Breakdown tab, in its fields too) |
| ⌘E | Ctrl+E | Export a report (File > Export…) |
| ⌘P | Ctrl+P | Print the Reports tab's report (File > Print…) |
| ⌘I | Ctrl+I | Explain the selected Day out of days row's counts (Edit > Explain This Count) |
| ⌘F, ⇧⌘F | Ctrl+F, Ctrl+Shift+F | Find; Find and Replace (Edit menu), from any field too |
| ⌘G, ⇧⌘G | Ctrl+G, Ctrl+Shift+G | Next and previous Find result (Edit > Find Next, Find Previous) |
| ↩, ⇧↩ | Enter, Shift+Enter | In the Find box: the next and previous result. Escape closes the panel |
| ↑ ↓ ↩ | Up, Down, Enter | In the Day out of days grid: select the row above or below, explain the selected row |
| ⌘U, ⌘., ⇧⌘D | Ctrl+U, Ctrl+., Ctrl+Shift+D | Board, Breakdown and Calendar tabs (View menu), the reference app's keys (decisions.md, entry 106) |
| ⌘3 ⌘4 ⌘6 ⌘7 ⌘8 | Ctrl+3, 4, 6, 7, 8 | Elements, Day out of days, Reports, Conflicts and Red flags tabs (View menu). The reference app's Reports key, ⌘P, is Print here, the platform's convention |
| ← → ↑ ↓ | Left, Right, Up, Down | In the Calendar's month: the day before or after, a week before or after |
| Page Up, Page Down | Page Up, Page Down | In the Calendar's month: the month before or after |

Board keys work on the Board tab while focus isn't in a text field; sheet keys work on the
Breakdown tab, in its fields too (they commit the field first). Both are provisional until
checked against the reference app (decisions.md, entry 2), and so are the View keys the
reference app has none for (⌘3, ⌘4, ⌘6, ⌘7, ⌘8; behavior note 21) and
⌘E, ⌘P, ⌘I, ⌘B, ⇧⌘B, ⌘D, ⌥⌘D, ⌘F, ⇧⌘F, ⌘G and ⇧⌘G (⌘F and ⇧⌘F are Method Budget's, which has
Find and Replace on ⇧⌘F rather than the common ⌥⌘F; ⌘G and ⇧⌘G are the Mac convention,
where Method Budget uses F3 because its ⌘G is a tab). Import Script, Export for Budget, Save as PDF, What Changed, What
Changed Since Saved, Merge Changes From, Edit Banner, Duplicate Strip, Merge Strips, Sort
Strips, Strip Layout, New Board, Duplicate Board, New Sub-board, Boards and Sub-boards, Red Flag Types
and the Breakdown menu's New Sheet, Duplicate Sheet and Delete Sheet have menu items and
no key. The Board menu's
commands show the Board tab first when another tab is open.

One table, `ui/js/shortcuts.js`, lists every shortcut; `src/menu.rs` builds the native menu
bar from the same ids and keys, and tests fail if they drift apart. Help > **Keyboard
Shortcuts** (⌘/) lists that table, grouped by menu and pane, with a filter, so it can't
drift either (`ui/js/keysheet.js`; decisions.md, entry 79). Help > **User Guide** shows
docs/GUIDE.md, built into the app, read-only in a dialog of the page's own with its
sections down the side (`ui/js/guide.js`; the Markdown is read by `guideBlocks` and
`guideRuns` in logic.js into text nodes, never HTML; no browser, no network; entry 199).
Help > **Open Sample** opens *Field Station*, the suite's invented feature (built into the
app from the engine, decision 223), as a new, untitled schedule titled "Field Station
(sample)" in this window, after the unsaved-changes question when anything is unsaved, as
New does; nothing built in is ever written, and the first Save asks where (entry 198). The
first launch with nothing to open shows it too, once, with a line saying what it is; a
person who opens their own file first never gets it (SUITE-STANDARDS.md section 8), and
neither does anyone who ran an earlier version (a settings file means they have). Help > **What's New** shows this
version's `docs/releases/<version>.md` the same way (build.rs embeds it; a version with no
notes says so). Help > **Report a Bug…** opens the repository's bug form in the browser with
the version and the computer filled in and nothing else (no file, path or schedule); Help
> **methodproduction.com** opens the suite's website; About's link opens methodschedule.com.
Those three addresses are the only ones the app opens (`open_link`, a fixed list in
src/help.rs, through the system's `open`, `xdg-open` or `rundll32`). Help > **Licenses and
Notices** shows LICENSE-MIT, LICENSE-APACHE and THIRD-PARTY-NOTICES.md, built in. **About
Method Schedule** (the app menu on macOS, the end of Help elsewhere) gives the version, the
website, the copyright line and the licence (entry 212). The reference app's keys are
used where its command exists here; where one of them is taken here (its Cmd G find
sheet, Cmd E quick entry, Cmd M layouts, Cmd Y breakdown window, Cmd 2 to 0 board and
calendar steps, Cmd ; preferences), ours stays and screen block 3 (A18) checks it. A menu item sends its
id to the page, which runs the same handler as the key. On macOS the page leaves app keys
(Save, Undo) to the menu's key equivalents; on Windows and Linux the page handles them and
drops a menu event that repeats a key it just handled. Quit is answered in Rust.

## Architecture

```
crates/method-schedule-app/
  src/session.rs   one open schedule: the Schedule, its path, the outside-change stamp,
                   undo and redo as whole-schedule snapshots (200 kept), a revision number
                   for the dirty flag, Session::edit (every change goes through it), open,
                   save, script import
  src/board.rs     the board edits as functions on &mut Schedule: move strips, day breaks
                   and banners (insert, edit, delete), sorting, automatic day breaks by
                   pages, the start date, and boards and sub-boards (add, duplicate,
                   rename, reorder, delete). Each validates what the page sent;
                   Session::edit makes it one undo step
  src/calendar.rs  the Calendar tab's edits on the picked sub-board: what a date is, the
                   weekly days off, events (added, replaced, deleted, checked)
  src/elements.rs  the Elements tab's edits: categories (add, rename, reorder, delete,
                   settings, custom properties, apply defaults) and elements (fields,
                   notes, property values, links, images, merge, move, renumber,
                   delete), with the engine's refusals as plain sentences
  src/reports.rs   the Reports tab's data for each report of the list with its options
                   (from method_schedule_core::reports, report_data and the day out of
                   days), and printing: the Save dialog for a PDF and the print
                   operation, through pdf_mac.rs on macOS
  src/reportsetup.rs  the report list and its options, the production info, the code
                   letters, prep and wrap dates (each edit one undo step), and the
                   user's preferences (week start, date format, calendar strip fields)
  src/pdf_mac.rs   macOS: AppKit's print operation on the web view, to the print sheet or
                   straight into a PDF file (Method Budget's, unchanged in substance)
  src/layouts.rs   the layout library: named layouts added, saved, renamed, moved,
                   deleted and picked (per board, and by the one-line report), and layout
                   files saved and loaded through Rust's dialogs
  src/export.rs    File > Export and Export for Budget: the report's workbook or CSV
                   (method_schedule_format::export) or the exchange file (method_schedule_format::exchange),
                   the Save dialog, the atomic write
  src/sheet.rs     the breakdown sheet edits, the same way: header fields, pages, quick
                   add (tag or create), remove, new, duplicate, merge (with its preview)
                   and delete sheet; and the strict readers for h:mm, m:ss, hh:mm and
                   pages plus eighths
  src/clipboard.rs cut, copy and paste: the app's own clipboard (in the session), strips
                   and elements copied as snapshots, pasted as a move (cut, same board)
                   or as copies, each paste one undo step; strips carried to another
                   window's schedule by element names (src/docs.rs shares a copy)
  src/explain.rs   Explain this count: method_schedule_core::explain for one element on the grid's
                   sub-board, as the panel draws it
  src/find.rs      Find and Replace: the query read and checked, results as the panel
                   lists them (place, context, where to go) under keys Replace takes back,
                   a replace as one undo step with its refusals as plain sentences
  src/conflicts.rs the Conflicts tab: method_schedule_core::conflicts for a board, with every
                   element, sub-board, sheet and strip named; the count the view carries
  src/redflags.rs  red flags: the type and flag edits (dates read strictly, refusals as
                   sentences), and method_schedule_core::redflags's clashes as the board's
                   strip marks and per-sub-board counts (only when some element has a
                   flag), the calendar's and day out of days' marks, and the Red flags tab
  src/plan.rs      the Plan menu, beyond the reference app: tagged import, script
                   revisions (picked by token), timelines and call times, locations,
                   turnarounds and rest, locks and drift, pins and the order suggestion,
                   call sheet data and the printed call sheet's data; each change one
                   undo step
  src/diffmerge.rs What Changed and Merge Changes From: the Open dialogs, the files picked
                   (by token), the base from the rolling backups, the preview and the
                   merge as one undo step
  src/outside.rs   changes made outside the app: the decision (a pure function), the check
                   on focus and before Save, Keep Mine, Load Theirs and Merge with the
                   file on disk; Revert to Backup
  src/view.rs      what the page draws: the board shown as day groups of strips with
                   totals, every board's name, one breakdown sheet, and the Elements tab,
                   day out of days grid and calendar of a named sub-board (serde shapes)
  src/safety.rs    plain files, no window (ported from Method Budget): atomic writes,
                   size-limited reads, the content hash and DiskStamp for the
                   outside-change guard, autosave timing, recovery copies with owner
                   locks, rolling backups, settings.json, recent.json
  src/safety_app.rs  autosave, saving, opening and recovery on the one session (tested
                   without a window), then the Tauri side: the 1-second autosave timer,
                   Open Recent and the Autosave check in the menu, open events and the
                   command line, the exit hook, and its commands
  src/app.rs       Tauri glue: the commands, native dialogs, the unsaved-changes question
                   on New, Open, close and Quit, the navigation guard, the capability test
  src/docs.rs      one schedule per window: the sessions by window label, and `Doc`, the
                   calling window's, as a command argument
  src/windows.rs   New Window, Open in New Window, Close Window and the Window menu
  src/menu.rs      the native menu bar, from one table shared with ui/js/shortcuts.js
                   (Open Recent and the Autosave check are entries in that table)
  src/lib.rs, src/main.rs
  build.rs         the command list that turns on Tauri's access control
  tauri.conf.json  window, CSP, capabilities, bundle icons
  capabilities/main.json  what the main window may call
  icons/           the app icons, the mark on slate blue, from scripts/make-icons.py
  ui/index.html, ui/styles.css
  ui/print.css     the printed pages: Letter-sized page boxes, header and footer bands,
                   the report tables; only #print-root shows when printing
  ui/js/api.js        invoke, through one queue; listen for menu events
  ui/js/dom.js        el() and friends: text only through textContent
  ui/js/dates.js      the date fields: text fields that take 3/1/2027 or 2027-03-01
  ui/js/shortcuts.js  every shortcut, one per line (read by the Rust tests)
  ui/js/main.js       state, the top bar, tabs, keys, menu commands, the strips' context menu
  ui/js/ctxmenu.js    a context menu drawn in the page (keys, submenus, placement)
  ui/js/modal.js      in-page dialogs (one at a time; the page behind is inert)
  ui/js/safety.js     the Autosave switch, the Recovered schedules dialog, Open Recent
                      and files from the system
  ui/js/outside.js    the outside-change question and the Revert to Backup dialog
  ui/js/board.js      drawing the board, the selection marks, dragging (drop indicator,
                      edge scrolling); where strips go when they move one place
  ui/js/boards.js     the board and sub-board bar, the Boards and Sub-boards dialog, and
                      the Banner and Sort Strips dialogs
  ui/js/layouts.js    the Layouts dialog: the named layouts, saved to and loaded from files
  ui/js/callsheet.js  the printed call sheet: the day's opening, then scenes, cast,
                      elements and notes as report.js flows (entry 207)
  ui/js/boardprint.js printing the board: its options dialog and its pages, through
                      report.js
  ui/js/colors.js     the Strip Colors dialog and the stylesheet that paints the strips in
                      the user's colors (checked `#rrggbb` only)
  ui/js/strips.js     the Merge Strips preview dialog; the Strip Layout dialog and the
                      column sizes the board and the one-line report draw a layout at
                      (main.js answers the cut, copy and paste events; breakdown.js
                      keeps the chosen elements)
  ui/js/breakdown.js  the Breakdown tab: the sheet's fields, the category boxes and quick add
  ui/js/elements.js   the Elements tab: categories, their settings, the element list
                      and editing one element
  ui/js/dood.js       the Day out of days grid: sticky columns and header, only the rows
                      in view drawn past 80 rows, the selected row and the Explain panel;
                      sorting, the hold and drop boxes, Without holds, Vertical, Codes
  ui/js/diffmerge.js  the What Changed and Merge Changes From dialogs
  ui/js/plan.js       the Plan menu's dialogs
  ui/js/find.js       the Find panel: the search form, the results, Next and Previous,
                      Replace with its checkboxes
  ui/js/conflicts.js  the Conflicts tab: conflicts, then event clashes and undated
                      sub-boards apart, each a link to its strip or date
  ui/js/redflags.js   the Red flags tab, the Red Flag Types dialog and the dialog that
                      adds or changes one element's flag
  ui/js/calendar.js   the Calendar tab: the month with its strips (dragged between days,
                      filtered), the picked date, the weekly days off, the preferences,
                      prep and wrap, and the event editor
  ui/js/reports.js    the Reports tab: the report list and each report's options, the
                      reports as tables from report_data, the preview, Print and Save as
                      PDF
  ui/js/report.js     paging: rows measured, then dealt onto Letter-sized pages with
                      their own header, column headings and footer; printing them
                      through print_report (Method Budget's report.js, scaled down)
  ui/js/export.js     the Export dialog (report and format) and Export for Budget
  ui/js/dynstyle.js   constructed stylesheets for sizes worked out at run time (Method
                      Budget's sheet.js)
  harness/            the page in a plain browser with a fake __TAURI__ (not embedded);
                      screens.html and screens-cli.html are the README and site pictures'
                      pages (scripts/capture-screens.mjs, decision 215)
  examples/           screen_answers.rs: the engine's answers those pictures show
```

**Rust owns the schedule.** The page holds no schedule: it sends a command and draws the
view it gets back. The session also remembers which board the window shows and what is
on the app's clipboard (neither saved nor undone); the page keeps which sub-board the per-sub-board tabs use and names it in
each call. Each window's `AppState` holds one `Session` behind a mutex; `src/docs.rs`
keeps them by window label, and every command takes `state: Doc`, the calling window's
(a test refuses Tauri's managed state of an `AppState`, which would belong to no window). `Session::edit` applies
an edit to a copy, refuses it whole if it fails or the result no longer passes
`Schedule::check`, skips it if nothing changed, and otherwise pushes the old schedule on
the undo stack, clears redo and takes a new revision. Unsaved changes means the revision
differs from the one last saved, so undoing back to the saved state is clean.

**Commands** (all in `src/app.rs`, listed in `build.rs`):

| Command | Arguments | Returns |
|---|---|---|
| `get_view` | | the view (the board shown, every board's id and name, `conflicts`: how many the board shown has, `red_flag_types`: `[{id, name, uses}]`; each sub-board carries `red_flags`, its clash count, and each clashing strip its clashes in words) |
| `dood_grid` | `sub_board`, `category` (may be null: Cast), `header` (a `reports::HeaderField` key: `set`, `location`, `unit`, `episode`, `int_ext`, `time_of_day`, `script_day`, `sequence`, or null), `sort` (up to four of `board_id`, `name`, `start`, `finish`, `work`, `hold`, each with `:desc`), `no_holds` | the day out of days grid: dates, rows of codes (the schedule's letters) with each row's summary and hold and drop switches; `codes` for the legend, `headers` for the picker, `excluded` when every element of the category is left off the board (no cell marked as a conflict) |
| `new_schedule` | | the view, or null when cancelled |
| `open_schedule` | | the view, or null when cancelled |
| `save_schedule` | `save_as` | the view, or null when cancelled |
| `import_script` | | the view with a notice, or null when cancelled |
| `undo`, `redo` | | the view |
| `move_strips` | `board`, `strips`, `sub_board` (null: the Boneyard), `index` | the view |
| `insert_day_break` | `sub_board`, `index` | the view, with `strip` naming the new day break |
| `delete_strips` | `board`, `strips` (day breaks and banners; a scene deletes its sheet on every board) | the view |
| `insert_banner` | `sub_board`, `index`, `text`, `duration` (`h:mm`, 24:00 at most, may be empty), `style` (`{bold, color, tall}`, may be null: plain) | the view, with `strip` naming the new banner |
| `set_banner` | `board`, `strip`, `text`, `duration`, `style` (null keeps it) | the view; a banner strip carries `bold`, `banner_color` and `tall` when set |
| `sort_strips` | `board`, `sub_board` (null: the Boneyard), `keys` (1 to 3 of `{field, descending}`) | the view |
| `continue_scene` | `board`, `strip` (a scene on a sub-board) | the view, with `strip` the new part; a part's strip carries `part` and `parts` |
| `add_layout` | `name`, `kind` (`strips`, `day_breaks`, `shoot_day`), `layout` (`{columns}` or `{look}`), `board` (null, or the board that picks it) | the view; it carries `layouts` (`[{id, name, kind, columns or look, boards, one_line}]`) and `one_line_layout`, the board its `own_layout`, `own_day_look`, `uses` and `shoot_text` |
| `save_layout`, `rename_layout`, `move_layout`, `delete_layout` | `id`, and `layout`, `name` or `to` | the view |
| `use_layout` | `board`, `kind`, `id` (null: its own) | the view |
| `use_one_line_layout` | `id` (null: the board's) | the view |
| `export_layouts` | `ids` (empty: all); shows the Save dialog | `{ done, file }` |
| `import_layouts` | shows the Open dialog | the view with a notice, or null when cancelled |
| `set_day_look` | `board`, `look` (`{shoot_day_strip, screen_time, sunrise, sunset, text}`) | the view; the board carries `day_look`, and each day its `screen_time`, `sunrise`, `sunset` and `full_date` |
| `set_strip_colors` | `rules` (`[{int_ext, time_of_day, color}]`, `#rrggbb`), `scene_text`, `banner_text`, `break_text` (each `#rrggbb` or null) | the view; the board carries `colors`, and a scene strip a color matches its `color` and `ink` |
| `set_strip_layout` | `board`, `columns` (1 to 15 of `{field, width}`, each field once) | the view; the board carries `layout` (`[{field, label, width}]`) and each scene strip its `cells` |
| `auto_day_breaks` | `sub_board`, `by` (`pages`, `time`, `screen_time`), `limit` (as typed) | the view, with a notice saying how many shoot days |
| `remove_day_breaks` | `sub_board` | the view, with a notice saying how many went |
| `set_start_date` | `sub_board`, `date` (as typed: `2027-03-01`, `3/1/2027` or `03/01/2027`; null or empty to clear) | the view |
| `show_board` | `board` | the view of that board (not an edit) |
| `add_board`, `duplicate_board` | `name`; `board`, `name` | the view, showing the new board |
| `rename_board`, `move_board`, `delete_board` | `board`, and `name` or `to` | the view |
| `add_sub_board` | `board`, `name` | the view, with `sub_board` naming the new one |
| `rename_sub_board`, `move_sub_board`, `delete_sub_board` | `sub_board`, and `name` or `to` | the view |
| `get_sheet` | `board`, `sheet`, `index` (both may be null) | the sheet (that one while it exists, else the one at `index` in the board's order), or null when there are none |
| `set_sheet_field` | `sheet`, `field`, `value` | the view |
| `set_sheet_pages` | `sheet`, `pages`, `eighths` | the view |
| `tag_element` | `sheet`, `element` | the view |
| `create_element` | `sheet`, `category`, `name` | the view |
| `untag_element` | `sheet`, `element` | the view |
| `new_sheet` | `board`, `after` (a sheet, or null) | the view, with `sheet` naming the new sheet |
| `delete_sheet` | `board`, `sheet` | the view, with `sheet` naming the one to show next |
| `copy_strips` | `board`, `strips`, `cut` | the view, its `clipboard` filled (not an edit) |
| `paste_strips` | `board`, `sub_board` (null: the Boneyard), `index` | the view, with `strips` the pasted strips |
| `copy_elements` | `sheet`, `elements`, `cut` | the view, its `clipboard` filled (not an edit) |
| `paste_elements` | `sheet` | the view, with a notice saying what moved or was added |
| `merge_sheets_preview` | `board`, `sheets` (two or more, in the order selected; the first survives) | `{survivor, merged, combined, elements_added, dropped}`: sheets as `{sheet, title}`, a combined field `{field, label, before, after}`, a dropped value `{sheet, title, field, label, value}`; nothing changes |
| `merge_sheets` | `board`, `sheets` (in the order selected) | the view, with `sheet` the surviving sheet and `strip` its strip on `board` |
| `duplicate_sheets` | `board`, `sheets` (one or more) | the view, with `strips` the copies' strips on `board` and `sheet` the last copy |
| `elements_view` | `sub_board`, `category` (may be null), `header` (a `reports::HeaderField` key: `set`, `location`, `unit`, `episode`, `int_ext`, `time_of_day`, `script_day`, `sequence`, or null) | the Elements tab: every category, the header fields, and that category's elements (the first category's when it is gone or null) or the header field's values; `all_elements` for the links editor |
| `element_days` | `sub_board`, `element` | `{element, name, sub_board, days: [{date, weekday, shoot_day, code, text}], empty}` |
| `add_property`, `rename_property` | `category`, `name`; `category`, `at`, `name` | the view |
| `move_property`, `delete_property` | `category`, `at`, `to`; `category`, `at` | the view |
| `set_property_value` | `element`, `name`, `value` (empty clears it) | the view |
| `set_links` | `element`, `links` (element ids), `retroactive` | the view, with a notice when `retroactive` |
| `add_element_image` | `element`; shows an Open dialog | the view, or null when cancelled |
| `remove_element_image` | `element`, `at` | the view |
| `element_image` | `element`, `at` | a `data:` URL of the image (PNG, JPEG, GIF, WebP, TIFF, HEIC or BMP by its bytes, 8 MB at most), or null when the file is missing, a link, a network share or not an image; read off the main thread |
| `set_quantity` | `sheet`, `element`, `count` (as typed), `daily` | the view |
| `add_category` | `name` | the view, with `category` naming the new one |
| `set_category_field` | `category`, `field` (`name`, `auto_board_id`, `exclude_from_board`, `daily_quantity`, `holds`, `drops`, `min_drop_gap`), `value` | the view |
| `move_category` | `category`, `to` (its place among the user categories) | the view |
| `delete_category` | `category` | the view |
| `apply_category_defaults` | `category` | the view, with a notice saying how many changed |
| `set_element_field` | `element`, `field` (`name`, `board_id`, `locked`, `holds`, `drops`, `min_drop_gap`, `notes`), `value` | the view |
| `merge_elements` | `survivor`, `others` | the view |
| `move_element` | `element`, `category` | the view |
| `renumber_elements` | `category`, `start` | the view |
| `delete_element` | `element`, `unlink` | the view |
| `red_flags_view` | `sub_board` | `{board, board_name, sub_board, sub_board_name, count, clashes, undated, flags, truncated}`: a clash is `{date, weekday, element, name, category, category_name, flag, type_name, dates, note, hard, day, sheets: [{sheet, scene, strip}], events: [{index, name}], text}`, at most 1,000 |
| `set_red_flag` | `element`, `flag` (`{id (null: new), red_flag_type, dates (range, not_before, not_after), from, to, note, hard}`) | the view |
| `delete_red_flag` | `flag` | the view |
| `add_red_flag_type`, `rename_red_flag_type` | `name`; `red_flag_type`, `name` | the view |
| `move_red_flag_type`, `delete_red_flag_type` | `red_flag_type`, and `to` | the view |
| `calendar_view` | `sub_board`, `year`, `month` (both, or both null for the start date's month), `week_start` (`sun` or `mon`), `fields` (the strips' fields) | one month of the sub-board's calendar in that week, its events, the elements an event can list, each shoot day's strips, pages, length and `drop_at` (its day break's place, for `move_strips`), `board`, `today`, `prep_start`, `wrap_end` |
| `set_day_kind` | `sub_board`, `date`, `kind` (`work`, `off`, `holiday`, `move`) | the view |
| `set_weekly_off` | `sub_board`, `days` (`mon` to `sun`) | the view |
| `save_event` | `sub_board`, `index` (null: a new event), `event` (`kind`: `travel`, `fitting`, `rehearsal`, `unavailable`, `camera_test` or `other`; `label`, `name`, `from`, `to`, `elements`) | the view |
| `delete_event` | `sub_board`, `index` | the view |
| `report_data` | `sub_board`, `index` (its place in the report list), `week_start` | the report's data for the sub-board with its options applied: its kind, name, title, header and production info, board, sub-board, start date, why it is empty if it is, and the days, elements, grids, totals, blocks, details, months, columns, units or tables it shows |
| `report_list` | | the report list (`index`, `name`, `kind`, `kind_label`, `landscape`, `options`), the production info, and the categories, elements and fields the pickers offer |
| `save_report` | `index`, `name`, `options` (`from`, `to` as typed, `landscape`, the switches, `categories`, `elements`, `header`, `text`, `columns`) | the view |
| `duplicate_report`, `move_report`, `delete_report` | `index`, and `to` | the view (a copy's notice names it) |
| `set_production` | `field` (`title`, `company`, `director`, `producer`), `value` | the view |
| `set_dood_codes` | `codes` (`{key: letters}` for the ones changed) | the view |
| `set_prep_wrap` | `sub_board`, `prep_start`, `wrap_end` (as typed; empty clears) | the view |
| `preferences`, `set_preferences` | `prefs` (`week_start`, `date_format`, `calendar_fields`, `fountain_lines`) | the user's preferences (settings.json) |
| `print_report` | `pdf` (true: into a PDF file chosen in a Save dialog), `landscape`, `title` (the job's name and the suggested file name), `subject` (optional: what it shows, for the PDF's properties) | `{ supported, done, file }`: `supported` false where the page prints with `window.print()` instead (not macOS); `done` false when cancelled; `file` the PDF's name |
| `export_report` | `sub_board`, `report` (`dood`, `one_line`, `elements`, `dood_grid`, `element_detail`, `breakdown`, `breakdown_day`, `board`, `totals`), `format` (`xlsx`, `csv`); shows the Save dialog | `{ done, file }`: `done` false when cancelled; `file` the name written |
| `export_budget` | `sub_board`; shows the Save dialog | `{ done, file }` |
| `set_autosave` | `on` | the setting |
| `open_pending` | | the view of a file the system asked to open, or null |
| `open_recent` | `index` (its place in Open Recent) | the view, or null when cancelled |
| `recovery_list` | | `[{ key, name, title, saved_at_ms }]`, newest first |
| `open_recovered` | `key` | the view, or null when cancelled |
| `discard_recovered` | `key` | the remaining list |
| `find_text` | `query` (`{text, match_case, whole_word, field, sub_board}`; `field` a key of `method_schedule_core::find::FieldFilter` or null for everything, `sub_board` null for the whole schedule) | `{count, hits, truncated}`: at most 1,000 `hits`, each `{key, kind, field, place, context, sheet, element, category, board, sub_board, strip, event, date}` in board order, `context` cut to 160 characters |
| `replace_text` | `query` (as above), `with`, `picks` (the checked results' keys) | the view, with a notice saying how many places changed |
| `conflicts_view` | `board` | `{board, board_name, count, conflicts, clash_count, event_clashes, undated, truncated}`: a conflict is `{element, name, category, date, weekday, bookings}`, a booking `{sub_board, sub_board_name, day, sheets: [{sheet, scene, strip}]}`, an event clash adds `events: [{sub_board, sub_board_name, index, name}]`; at most 1,000 of each |
| `explain_count` | `sub_board`, `element` | the Explain panel for that element on the sub-board: name, category, totals, the summary sentences, `gaps` (`{after, before, days, outcome, text}`) and `days` (`{date, code, counted, text}`) |
| `what_changed` | `against` (`file`: an Open dialog; `saved`: the schedule's own file on disk) | `{from, to, count, groups: [{group, changes: [{kind, subject, text, go}]}]}`, or null when cancelled; `go`, where a change still in the window is (a Find result's shape: `kind` `sheet`, `element`, `strip`, or `event` with `sub_board` and `date` for a calendar date or event), is left out otherwise and in the merge's list (decisions.md, entries 235 and 237) |
| `merge_pick_file` | shows the Open dialog | `{token, name}`, or null when cancelled |
| `merge_preview` | `token`, `base` (`{kind: "auto"}`, `{kind: "disk"}`, `{kind: "last_read"}` or `{kind: "backup", name}`), `picks` (`{key: "ours" \| "theirs"}`) | `{theirs, ours, base, base_label, base_reason, base_clear, bases, conflicts, count, changes, renumbered, fallbacks, problem}`; a conflict is `{key, what, base, ours, theirs, taken, text}` |
| `merge_apply` | `token`, `base`, `picks` | the view, with a notice saying how the conflicts were settled |
| `merge_preview`, `merge_apply` in the browser host | `file` (theirs), `base` (`"file"`, the default, with `base_file`; or `"saved"`, the schedule as last opened or saved there), `picks`; `merge_apply` also `pictures` (theirs', as `open_schedule` takes them) | the preview `{theirs, ours, base, conflicts, count, changes, renumbered, fallbacks}`, the base being its label; the apply as above (decisions.md, entry 239) |
| `outside_check` | `force` (true before Save: ask again about a version put off) | null, or `{name, version, changed_at, dirty, choices, default, confirm_load, problem, token}`: `choices` of `merge`, `keep_mine`, `load_theirs`; `token` is Merge's, for `merge_preview` and `merge_apply` (base `{kind: "auto"}` or `{kind: "last_read"}`) |
| `outside_resolve` | `choice` (`keep_mine`, `load_theirs`, `later`), `version` (as `outside_check` gave it) | the view, with a notice; refused when the file changed again since |
| `backup_list` | | `{file, backups: [{name, label, changes}]}`, newest first; `changes` null when one can't be read |
| `revert_to_backup` | `name` (a backup's file name from `backup_list`) | the view, with a notice |
| `import_script_tags` | shows the Open dialog (`.fdx`) | the view with a notice, or null when cancelled (Plan menu; decisions.md, IMP-04) |
| `revise_pick` | `tags`; shows the Open dialog | `{token, file, matched, changed, added, omitted, lines}`, the revision worked out on a copy, or null when cancelled |
| `revise_apply` | `token` | the view: the revision merged as one undo step, recorded as the next draft when it changed anything; an omitted scene's strip (and its report lines) carry `omitted` and are drawn struck |
| `conflicts_data` | | `[{key, what, base, ours, theirs, taken, date, go}]`, the merge conflicts kept for review, oldest first; `go` as What Changed's, left out for something no longer there (decisions.md, entry 238) |
| `settle_conflicts` | `keys` (none for every one) | The view; the conflicts with those keys leave the review list (entry 238) |
| `drafts_data` | | `[{color, date, changed, added, omitted}]`, every script draft merged, oldest first; the scene lists are names such as `Scene 12, INT. BOAT` (Plan > Script Drafts; decisions.md, entry 233) |
| `timeline_data` | `sub_board`, `strip` (may be null) | `{sub_board, sub_board_name, long_day, days, focus}`: each day `{number, date, day (its day break), call, wrap, length, sun, entries: [{strip, kind, text, start, end, untimed, warnings}], warnings}`; `focus` the day holding `strip`; each day also `past_wrap` (what runs past its wrap, in words, empty when nothing does), `push` (those strips, empty on the last day) and `next` (decisions.md, entry 192), and `people`: `[{element, name, call, wrap}]`, who works the day with their own call and wrap, empty for the company's (entry 231) |
| `push_past_wrap` | `sub_board`, `day`, `strips` (the day's `push` as previewed) | the view, with a notice: the strips moved to the start of the next day as one undo step (refused when the day changed since) |
| `set_call_time`, `set_long_day` | `day`, `call` (`h:mm`, empty clears); `value` (`12`, `10:30`, empty: 12 hours) | the view |
| `set_person_call` | `day`, `element`, `call`, `wrap` (`h:mm` each, empty for the company's; a wrap at or before the call is after midnight) | the view (decisions.md, entry 231) |
| `locations_data` | | `[{name, sheets, latitude, longitude, utc_offset, daylight_saving}]`, every sheet location; `daylight_saving` `us_canada`, `eu`, `australia`, `new_zealand`, `israel`, `egypt`, `chile` or empty for none |
| `set_location` | `name`, `latitude`, `longitude`, `utc_offset` (as typed; both degrees empty takes it off the map), `daylight_saving` (the select's value: `us_canada`, `eu`, `australia`, `new_zealand`, `israel`, `egypt`, `chile` or empty; decisions.md, entries 193, 220 and 230) | the view |
| `rest_data`, `set_turnaround` | `board`; `category`, `value` | `{turnarounds: [{category, name, minimum}], short, unchecked}`; the view |
| `locks_data`, `lock_schedule`, `delete_lock`, `drift_data` | `board`; `board`, `name`; `name`; `name` | `[{name, made, scenes}]`; the view; the view; `{lock, actors: [{name, board_id, before, after, added, dropped, moved}], unchanged}` |
| `pin_strips` | `strips`, `pinned` | the view, with a notice |
| `suggest_order`, `apply_suggested_order` | `sub_board`; `sub_board`, `order` (as previewed) | `{sub_board, sub_board_name, order, moved, pinned, groups, rows: [{label, before, after}], rule, tried: [{rule, cost}]}` (the cheapest of three orders, or the board's own when none costs less; decisions.md, entry 234); the view (refused when the board changed since) |
| `export_call_sheet`, `shoot_day_count` | `sub_board`, `day`, `format` (`json`, `csv`); shows the Save dialog; `sub_board` | `{done, file}`; the number of shoot days |
| `call_sheet_data` | `sub_board`, `day` | the call sheet JSON object (docs/CALLSHEET.md), for Plan > Print Call Sheet (decisions.md, entry 207) |

Rust tells the page about saving with events: `file-state` (the view's `file` part, after
an autosave), `autosave-changed` (the switch, from the menu), `autosave-error` (said once
per problem), `open-request` (a file from the system; the page waits for any dialog,
then calls `open_pending`) and `outside-check` (the window got focus; the page calls
`outside_check` unless a dialog is open). The view carries `autosave`, so the switch starts right.

Sheet values travel as text and are read in Rust (`src/sheet.rs`); `field` must be one of
the header fields' names, anything else is refused. The page asks before `delete_sheet`.
Elements tab values travel the same way (`src/elements.rs`): switches as `true` or
`false`, numbers as typed and read strictly. The page asks before
`apply_category_defaults`, `merge_elements` (which one survives), `renumber_elements` (the
start), `delete_element` (and whether to unlink) and `delete_event`. Dates travel as
typed and are read in Rust (`src/calendar.rs`, `typed_date`: `2027-03-01`, `3/1/2027`
or `03/01/2027`, nothing else; the views send them back as `YYYY-MM-DD`); an event names elements
by id, each checked. A red flag's dates travel the same way, read strictly in
`src/redflags.rs`; the page asks before `delete_red_flag` (deleting a type takes a second
click in its dialog). The Elements view lists each element's flags, the calendar's dates
and the day out of days rows carry their clashes in words.

Board commands name their board or sub-board (the command line and the MCP server still
use the first board). `move_strips` takes positions counted before the move
(`Schedule::move_strips`), keeps the strips in board order, and refuses to put a day break
or banner in the Boneyard. Banner text is capped at 200 characters and its duration read
strictly as `h:mm`, 24:00 at most (a shoot time too); sort fields must be the ten the engine knows, each once, at most three.
The page asks before `delete_board`, `delete_sub_board` and deleting banners. Errors come
back as one plain sentence the page shows in its status line (or in the open dialog).

## Updates and releases

One version, in the root `Cargo.toml` (`[workspace.package]`); the bundles take it from
there. Bundles are made with the Tauri CLI, pinned at 2.12.1 (no npm):

```bash
cargo install tauri-cli --locked --version 2.12.1
```

**A Mac release, on the owner's Mac** (decisions.md, entry 196):

```bash
scripts/release-mac.sh
```

It builds the release `Method Schedule.app` and `.dmg` with the Tauri bundler, signed with
the Developer ID in `METHOD_SIGN_IDENTITY` (default the suite's Developer ID, by its team ID
`XSRB3D643J`) with the hardened runtime and a
secure timestamp, signs the `.dmg`, checks both with `codesign --verify --strict --deep`,
then notarizes the `.dmg` with the notarytool keychain profile in `METHOD_NOTARY_PROFILE`
(default `method-schedule-notary`), staples it and the app, and checks them with `spctl
--assess` and `xcrun stapler validate`. The results are in `release-mac/` in cargo's build folder (`CARGO_TARGET_DIR`, else `target/`); nothing
is uploaded but to Apple's notary service. `METHOD_SKIP_NOTARIZE=1` stops after signing;
`METHOD_MAC_TARGET=universal-apple-darwin` builds for Intel and Apple silicon (with both
Rust targets installed). No entitlements file: the app needs none under the hardened
runtime.

The notary profile is made once, by the owner, who types the app-specific password at the
prompt (never on the command line):

```bash
xcrun notarytool store-credentials method-schedule-notary \
  --apple-id <the Apple ID> --team-id XSRB3D643J
```

Without it the script stops before sending anything and prints that command.

**Draft releases from CI**: `.github/workflows/release.yml` runs on a `v*` tag that matches
the version and points at a commit on main (decision 210). It builds the Mac app for Apple
silicon only (`.dmg`), the Windows NSIS installer and the Linux `.deb` and AppImage, renames
them to version-free names so the website can link `releases/latest/download/<file>`
(`Method-Schedule-macOS.dmg`, `Method-Schedule-windows-x64-setup.exe`,
`Method-Schedule-linux-amd64.deb`, `Method-Schedule-linux-x86_64.AppImage`), and drafts a
GitHub release with them and `docs/releases/<version>.md` as the notes. Signing uses the
suite's secret names, Method Budget's: `DEVELOPER_ID_CERT_P12_BASE64`, `CERT_PASSWORD` and
`KEYCHAIN_PASSWORD` sign the Mac build (the identity is the one the certificate holds),
`AC_API_KEY_ID`, `AC_API_ISSUER_ID` and `AC_API_KEY_P8_BASE64` notarize it, and
`WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` sign the Windows installer. Each
step is skipped without its secrets; none are set. The first Windows alpha ships unsigned
(SmartScreen warns). Publishing a draft is by hand.

**The updater** (`src/updater.rs`, `ui/js/updates.js`; decisions.md, entry 213), Method
Budget's: two minutes after launch and then every six hours, unless Help > **Check for
Updates Automatically** is off (on by default; `updates.json` in the data folder), the app
fetches `https://github.com/method-production-suite/method-scheduling/releases/latest/download/latest.json`.
A newer version downloads in the background; the plugin checks its minisign signature
against the public key in `tauri.conf.json` before keeping it; then the footer shows
**Update ready** with **Restart to Update**, which goes the way Quit does (each window's
unsaved changes get the question), installs and starts again. Help > **Check for
Updates…** checks now and answers in the status line; an automatic check says nothing.
Requests stay on https and on github.com and its two download hosts, redirects included.
What's New shows once after an update. macOS only for now.

**The key** (decision 214): the owner's minisign key `460428DB6D38CE76`, made with
`cargo tauri signer generate -w ~/.tauri/method-schedule-updater.key`. The public key is
`plugins.updater.pubkey` in `tauri.conf.json` (and `~/.tauri/method-schedule-updater.key.pub`
on the owner's Mac); the private key and its password are in 1Password as "Method Schedule
updater key" and nowhere in this repository. With the key in, a Mac build checks: two
minutes after launch it asks GitHub for `latest.json`. While the repository is private
GitHub answers "not found", which an automatic check keeps to itself, so nothing shows.

What else turns updates on, all the owner's:

1. **The release secrets**: `TAURI_SIGNING_PRIVATE_KEY` (the text of the private key file)
   and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` in the repository's Actions secrets. With them,
   `release.yml` makes the signed `Method-Schedule_<version>_universal.app.tar.gz` (one
   app for Intel and Apple silicon, decision 225) and `scripts/update-manifest.py` writes
   `latest.json` into the draft, listing it under both `darwin-aarch64` and `darwin-x86_64`.
2. **The Apple signing secrets** (`DEVELOPER_ID_CERT_P12_BASE64`, `CERT_PASSWORD`,
   `KEYCHAIN_PASSWORD`, `AC_API_KEY_ID`, `AC_API_ISSUER_ID`, `AC_API_KEY_P8_BASE64`), so the
   build an update installs is signed and notarized like the first one.
3. **A public repository** (decision 197): installed apps download `latest.json` and the
   archive without signing in to GitHub, which only works once the repository is public.
4. **A published release**: bump the version, tag `vX.Y.Z` on main, let `release.yml` draft
   it, check the draft (it must hold `latest.json`), then publish it as a full release
   (`releases/latest` skips drafts and pre-releases).

Only copies built with this key can update: the first build people install must already
carry it, which every build from decision 214 on does.

## Security model

The app opens files the user picks and nothing else, and the web view is treated as
untrusted (a schedule or a script is data from anywhere).

- **No network but the update check.** No telemetry, no remote content, no accounts. The
  updater (below) fetches `latest.json` from the GitHub release over https and nothing
  else, and can be turned off (Help > Check for Updates Automatically); Help's links open in the browser, not in the app. The CSP limits every resource to the app's own
  bundled files and Tauri's IPC, as strict as Method Budget's but for one thing:
  `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`.
  `data:` images are allowed for the thumbnails of element and sheet images: Rust reads
  the file at a path the schedule stores (put there by a Rust Open dialog, never by the
  page; a schedule from elsewhere may name any path, so only plain files of 8 MB or less
  whose first bytes are a PNG, JPEG, GIF, WebP, TIFF, HEIC or BMP image are read, never a link, a network
  share or a device path) and hands the page a `data:` URL; an image can't run script,
  and the page has no way to send it anywhere.
  The Add Image dialog lists all seven types. macOS's WebKit draws HEIC and TIFF, so
  their thumbnails show there; the web views on Windows and Linux may not draw HEIC,
  TIFF or BMP, and where one fails to load the thumbnail shows the file's name instead
  (`showThumb` in `ui/js/dom.js`). The schedule keeps the path either way.
  No inline scripts or styles, no CDN. Strip colors, selection and the grid's shading are
  classes. The one size worked out at run time, the height of the spacer rows around the
  day out of days rows in view, goes into a constructed stylesheet (`ui/js/dynstyle.js`,
  as Method Budget does): CSSOM, outside the CSP's reach, and only numbers go into it.
  The Reports tab's preview scale and the printed page size (`@page`) go the same way,
  and so do the strip layout's column widths on the board and the one-line report's
  (fixed words such as `5em` and `minmax(9em, 1fr)` picked by the width's name, and
  percentages; never schedule text).
- **No clipboard permission.** Cut, Copy and Paste keep what they hold in Rust (the
  session's clipboard), named by nothing the page sends but the board, sheet, strip and
  element ids it acts on. The page puts plain text on the system clipboard only from
  the DOM's own copy and cut events (`clipboardData`), which the user's key or menu
  choice starts and which need no capability; it never reads the system clipboard.
- **Navigation is locked** to the bundled UI (`tauri://localhost`, or
  `http://tauri.localhost` on Windows) by a small navigation guard plugin, so a link or a
  dropped URL can't load another page.
- **Least privilege for the web view.** `capabilities/main.json` grants the main window
  and the schedule windows opened after it (`doc-*`)
  exactly the commands its scripts call, plus `core:event:allow-listen` for the
  menu's events, and nothing else. `build.rs` lists the commands in an app manifest, which
  turns on Tauri's access control for them: anything not listed is refused. tauri-build
  writes one permission file per command into `permissions/autogenerated/` on every build
  (gitignored, as in Method Budget). A test in `src/app.rs` reads `build.rs`,
  `generate_handler!`, the capability and every `invoke('...')` in `ui/js/` and fails on
  any difference among the four, or on any other permission. There is no fs or shell
  plugin, and the page gets no dialog permissions. (`tauri-plugin-dialog` depends on the
  fs plugin crate as a library, but it is never registered, so none of its commands exist.)
- **Paths never come from the web view.** Open, Open in New Window, Save As, Import
  Script, Export, Export for Budget, Export Call Sheet Data, Save as PDF, Add Image (an
  element's or a sheet's), Save as Template and New from Template, Save Layouts and Load
  Layouts, What Changed and Merge Changes From show the native dialog from Rust
  (`tauri-plugin-dialog`) and read or write the chosen file there. Every Save dialog but
  Save as Template (which opens in the templates folder) opens in the schedule's folder,
  or, before the schedule is saved, where the system last saved (entry 171). The page names a
  report and a format, or a print
  job's title, never a path, and gets back file names only. The
  page can ask to open, save or import, but it can't name a path, and the view it gets
  carries the file's name, never its folder. The other paths come from the operating
  system (open events, the command line), the recent list and the recovery folder: the
  page names a recent file by its place in the list and a recovery copy by its key, which
  must be a plain name (lowercase letters, digits and `-`) listed in `Recovery/` by a copy
  of the app that is no longer running. The recovery list carries file names, not paths.
  A file to merge from is kept in Rust and named by a token; a merge base is a backup
  named by its file name, which must be one the backups folder lists for this schedule,
  or `disk`, the schedule's own file.
- **The data folder is private.** Recovery copies, backups, `settings.json` and
  `recent.json` are written atomically, readable only by the user (0700 folders, 0600
  files on Unix). A recovered schedule goes through the same strict reader and
  `Schedule::check` as any file. Saves write a temporary file next to the
  target, flush it and rename it into place (and flush the folder on Unix), so a failed
  save never leaves half a file. Files over 64 MB are refused before they are read.
- **The outside-change guard.** The session remembers a hash of the file as it last read or
  wrote it. Before every save over that file, the file is read and hashed again; if it
  holds anything else (a sync service, another app, the command line changed it), nothing
  is written and the page asks how to settle it (above). A file that was moved or deleted
  is written again. Save As to another file is not guarded: the Save dialog has asked about
  replacing it.
- **Exports and PDFs.** An export is written atomically and is refused when the chosen
  file is the open schedule. A PDF is printed into a hidden temporary file beside the
  chosen one and renamed into place when the print operation is over, so a failed or
  cancelled job leaves the old file alone. Text that a spreadsheet would run as a formula
  is neutralized by `method_schedule_format::export` (CSV) or written as a string (workbook).
- **Printing touches only the page.** Printing hands the web view of the window that
  asked (each schedule window prints its own) to the system's print operation
  (`src/pdf_mac.rs`, the app's only `unsafe` code: plain Objective-C
  runtime calls, Method Budget's). The page to print is laid out with the same DOM calls
  and `textContent` as everything else.
- **Schedule and script text is never markup.** The page builds every node with DOM calls
  and puts every string in with `textContent` or an input's value, never `innerHTML`.
  `freezePrototype` is on.
- **What the page sends is checked in Rust.** Find's search text (1,000 characters), field
  (one of the engine's keys) and sub-board are checked; Replace takes results back as
  keys (`sheet:12:set`), each parsed strictly (plain digits, a known field, at most 64
  bytes), and the engine refuses any that isn't a result of the same search any more; the
  replacement is capped at 10,000 characters. Ids must name what is there, a sheet field
  must be one of the header fields, times and pages are parsed strictly as integers, and
  text is capped (10,000 characters for header fields, 200 for names). Every edit runs on
  a copy and must pass `Schedule::check` before it lands.
- **Hostile files** can't crash the engine: the schedule reader is strict and checks the
  whole schedule (FORMAT.md), and the `.fdx` reader refuses DTDs and caps size, depth and
  counts (decisions.md, entry 13). An edit that would leave the schedule inconsistent is
  refused before it lands.

## Dependencies

Tauri crates are pinned exactly, at the versions and features Method Budget uses:
`tauri` 2.12.1 with `default-features = false` and only `wry`, `common-controls-v6`, `x11`
and `dbus`; `tauri-build` 2.7.1; `tauri-plugin-dialog` 2.8.1. `serde` 1.0.229 and
`serde_json` 1.0.151 as elsewhere. Every crate in `Cargo.lock` is a version Method Budget's
lock already has, and must be at least 14 days old (`python3 scripts/check-crate-ages.py`,
run in CI).

## Live checks

Screen block 6, 2026-10-17 (docs/research/screen-block-6.md): the debug app bundle on a
ZTEST schedule passed the day break sun from a located sheet, the calendar filter's
Escape, Locations' Return and Close, the call sheet save folder, every outside-change
branch with Revert to Backup, copying strips between windows, the Breakdown Designer drag,
the printed board's density, daylight saving on a placed location, category elements on
strips, look A with Graphite and Ledger, Print Call Sheet, the Help menu and opening a
template from the system. Four look bugs it found are fixed (decisions.md, entry 218).

First run in a real window, 2026-10-11, macOS (debug build wrapped in a minimal app bundle
so computer use could drive it), on a ZTEST schedule built from an invented script:
- Opening a `.mtsc` file from macOS (`open -a`), the native File and Edit menus, the
  Import Script sheet, the board with dates and day breaks, Option+Up and Option+Down
  across day breaks, the Breakdown tab's fields, the DOOD summary (matches the CLI and
  ZTEST run 1), undo.
- Autosave wrote an edit to disk within seconds.
- Changing the file on disk paused autosave, kept the work in a recovery copy and left the
  other version alone. Found and fixed: the warning was a passing status message that the
  next redraw cleared; it is now a standing banner driven by `file.conflict`, cleared by a
  successful save.
- A forced kill, then relaunch: Recovered schedules offered the work; opening it kept the
  unsaved state; the first Save asked before replacing the file.
- Quit with unsaved changes asked Save, Don't Save or Cancel; Don't Save removed the
  recovery copy. A lock file stayed in `Recovery/` after the clean quit. Fixed: Tauri never
  drops its managed state on exit, so the exit hook now releases and removes the lock
  (decisions.md, entry 18); to confirm in the next live run.

## Not yet

Brought up to date on 2026-10-13 (feature review): built since the list was written and
taken off it are banner sizes and alignment, the strip designer's later lines, custom
text, merged cells and alignment, the MCP server choosing a board, the red flag table
with its element filter and one flag for several elements, report layouts in layout
files, the daily category totals and the one-line spreadsheet following the strip
layout, printing the board, Find matches highlighted on the board, and the reference
app's key equivalents (behavior note 19).

- **Single instance handling** on Windows and Linux (a second launch with a file starts a
  second copy), and the system-wide recent documents list.
- **The board**: a Paste Special (as a move, or above the selection), the rest of the strip designer (scaling to the content; entries 87 and 185), a
  category's elements more than once on one layout (entry 194), sending only what an edit changed (the
  whole board view is still sent; at 1,500 strips the view and its JSON cost about 3 ms,
  entries 24 and 39; a budget test holds an edit plus the view under 5 ms, entry 176)
  (the command line chooses a board with `--board` since entry 132).
- **The split screen**: dragging strips between the board and a pane beside it (another
  board, or a calendar date).
- **More of red flags**: flag notes in Find (decisions.md, entries 33 and 36; the table
  of every flag and adding to several elements at once came with entry 114).
- Dragging whole days on the calendar.
- **More of the breakdown**: splitting a sheet (Duplicate Sheet is the way for now);
  pasting elements onto several sheets at once; a custom drag order in the Elements tab;
  links by episode for series; an image marked for reports; property values in Find and
  the strip designer.
- **More of the day out of days**: the minimum drop gap from a row, and explaining one
  cell from a right-click.
- **More of reports and export**: the rest of the report designer (dragging fields,
  named fonts, footer fields); paper sizes other than Letter; Save as PDF without the
  dialog on Windows and Linux; the shooting schedule as a spreadsheet; tab-delimited
  text (the writer has it; no export offers it yet).
- **More of Find and Replace**: red flag notes, replacing in the Boneyard's scenes only,
  and a find bar per tab.
- **More of What Changed and merging**: going to a change from inside the merge itself
  (What Changed links sheets, elements, strips and calendar dates, decisions.md entries
  235 and 237); picking any file as a merge base;
  the changes a merge brings in shown per conflict
  pick side by side.
- **Beyond the reference app**: daylight saving rules other than the US and Canada's, the EU's,
  Australia's south-east, New Zealand's, Israel's, Egypt's and mainland Chile's (a location
  with none keeps one UTC offset; decisions.md, entries 193 and 230), and
  sending call sheets (it needs a service, BYD-03).
- **Updates and releases**: the release and Apple signing secrets in CI, a public
  repository and a first published release (the updater and its key are in, entries 213
  and 214; "Updates and releases" above), updates on Windows and Linux, and notarizing the
  first build (it waits for the owner's notary profile, decision 196).
