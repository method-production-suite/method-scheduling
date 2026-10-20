// Every keyboard shortcut in the app, in one list. The native menu bar (src/menu.rs) uses
// the same ids and keys; a Rust test reads this file and fails if the two drift apart, so
// keep one entry per line.
//
// Keys are written once for both platforms: `mod` is Command on macOS and Ctrl on Windows
// and Linux; `alt` is Option on macOS. An entry with `mac` and `other` instead of `keys`
// differs by platform; a list gives alternatives (the first is the one menus show).
//
// scope says who answers the key:
//   app    The menu item's key equivalent on macOS (main.js leaves the key alone there);
//          main.js on Windows and Linux.
//   board  main.js, on the Board tab while focus isn't in a text field. No menu item: a
//          bare key as a menu key equivalent would fire while typing.
//   sheet  main.js, on the Breakdown tab, in its fields too (the keys have a modifier).
//          No menu item yet.
//   menu   A menu command with no shortcut. file.autosave is answered in Rust, like
//          app.quit; File > Open Recent's items (file.recent.<n>) are built from the
//          recent list, not this table.
//   page   A key a pane answers itself (the strips' context menu, the calendar's
//          arrows, the Find box), listed here so the shortcut sheet shows it; never
//          matched by main.js and never a menu item.
//
// Help > Keyboard Shortcuts (keysheet.js) lists this table, so the sheet never drifts
// from the keys (decisions.md, entry 79). Keys follow the reference app where its
// action exists here (inventory section 14, behavior note 10): Close Window (mod+w),
// Go to Scene (mod+j, the older app's find scene), Linked Elements (mod+l) and Add Red
// Flag (mod+k). Where its key is taken here by another command, ours stays and the clash
// is listed for screen block 3 (A18).
// The File, Edit and Schedule keys were checked in the reference app (behavior note 21):
// New, Open, Save, Save As, Close, Undo and Redo match; its Board (mod+u), Breakdown
// (mod+.) and Calendar (mod+shift+d) keys are ours too; its Reports key (mod+p) stays
// Print here (decisions.md, entry 106). Board and sheet keys are provisional until
// checked (decisions.md, 2), and so are Export (mod+e), Print (mod+p), Explain (mod+i), the
// banner keys (mod+b, mod+shift+b), Find's (mod+f, mod+shift+f, mod+g, mod+shift+g) and
// the Red flags tab's (mod+8).

import { isMac } from './dom.js';

export const SHORTCUTS = [
  // File
  { id: 'file.new', label: 'New schedule', keys: 'mod+n', scope: 'app' },
  { id: 'file.new_window', label: 'Open a new window with a new schedule', keys: 'mod+shift+n', scope: 'app' },
  { id: 'file.open', label: 'Open a schedule', keys: 'mod+o', scope: 'app' },
  { id: 'file.open_window', label: 'Open a schedule in a window of its own', scope: 'menu' },
  { id: 'file.save', label: 'Save', keys: 'mod+s', scope: 'app' },
  { id: 'file.save_as', label: 'Save as a new file', keys: 'mod+shift+s', scope: 'app' },
  { id: 'file.autosave', label: 'Turn autosave on or off', scope: 'menu' },
  { id: 'file.new_from_template', label: 'Start a new schedule from a template', scope: 'menu' },
  { id: 'file.save_template', label: "Save this schedule's setup as a template (no sheets, strips, events or red flags)", scope: 'menu' },
  { id: 'file.revert_backup', label: 'Revert to one of the backups kept of this schedule file', scope: 'menu' },
  { id: 'file.import_script', label: 'Import a script (Fountain, .fdx or .mscenes) into the Boneyard', scope: 'menu' },
  { id: 'file.what_changed', label: 'What changed from another schedule file to this one', scope: 'menu' },
  { id: 'file.what_changed_saved', label: 'What changed since this schedule was last saved', scope: 'menu' },
  { id: 'file.merge', label: 'Merge changes from another copy of this schedule', scope: 'menu' },
  { id: 'file.export', label: 'Export a report as a spreadsheet (.xlsx) or CSV', keys: 'mod+e', scope: 'app' },
  { id: 'file.export_budget', label: 'Export the day counts for a budget', scope: 'menu' },
  { id: 'file.print', label: 'Print the board (Board tab) or the report on the Reports tab', keys: 'mod+p', scope: 'app' },
  { id: 'file.save_pdf', label: 'Save the board (Board tab) or the report on the Reports tab as a PDF', scope: 'menu' },
  { id: 'file.close', label: 'Close this window (asks about unsaved changes)', keys: 'mod+w', scope: 'app' },
  { id: 'app.quit', label: 'Quit', keys: 'mod+q', scope: 'app' },
  { id: 'app.settings', label: 'Settings: week start, date format and Fountain lines per page, for this computer', keys: 'mod+,', scope: 'app' },

  // Edit
  { id: 'edit.undo', label: 'Undo', keys: 'mod+z', scope: 'app' },
  { id: 'edit.redo', label: 'Redo', mac: 'mod+shift+z', other: ['mod+y', 'mod+shift+z'], scope: 'app' },
  { id: 'edit.explain', label: "Explain the selected Day out of days row's counts", keys: 'mod+i', scope: 'app' },
  { id: 'edit.find', label: 'Find in the schedule', keys: 'mod+f', scope: 'app' },
  { id: 'edit.replace', label: 'Find and replace in the schedule', keys: 'mod+shift+f', scope: 'app' },
  { id: 'edit.find_next', label: 'Go to the next Find result', keys: 'mod+g', scope: 'app' },
  { id: 'edit.find_previous', label: 'Go to the previous Find result', keys: 'mod+shift+g', scope: 'app' },
  { id: 'edit.red_flag_types', label: 'Add, rename, reorder and delete red flag types', scope: 'menu' },
  { id: 'edit.links', label: 'Link elements to the element picked on the Elements tab', keys: 'mod+l', scope: 'app' },
  { id: 'edit.add_red_flag', label: 'Add a red flag to the element picked on the Elements tab', keys: 'mod+k', scope: 'app' },

  // View (the tabs). Board, Breakdown and Calendar take the reference app's keys
  // (behavior note 21; decisions.md, entry 106); its Reports key (mod+p) is Print here, a
  // platform convention, so Reports keeps mod+6. The others are ours (it has none).
  { id: 'view.board', label: 'Show the Board tab', keys: 'mod+u', scope: 'app' },
  { id: 'view.breakdown', label: 'Show the Breakdown tab', keys: 'mod+.', scope: 'app' },
  { id: 'view.elements', label: 'Show the Elements tab', keys: 'mod+3', scope: 'app' },
  { id: 'view.dood', label: 'Show the Day out of days tab', keys: 'mod+4', scope: 'app' },
  { id: 'view.calendar', label: 'Show the Calendar tab', keys: 'mod+shift+d', scope: 'app' },
  { id: 'view.reports', label: 'Show the Reports tab', keys: 'mod+6', scope: 'app' },
  { id: 'view.conflicts', label: 'Show the Conflicts tab', keys: 'mod+7', scope: 'app' },
  { id: 'view.red_flags', label: 'Show the Red flags tab', keys: 'mod+8', scope: 'app' },
  // Split screen (decisions.md, entry 80): a pane beside the board. Provisional keys.
  { id: 'view.split_breakdown', label: "Show the selected strip's breakdown sheet beside the board", keys: 'mod+shift+2', scope: 'app' },
  { id: 'view.split_board', label: 'Show another board or sub-board beside the board', keys: 'mod+shift+1', scope: 'app' },
  { id: 'view.split_calendar', label: 'Show the calendar beside the board', keys: 'mod+shift+5', scope: 'app' },
  { id: 'view.split_images', label: "Show the selected scene's images beside the board", scope: 'menu' },
  { id: 'view.split_breakdown_images', label: "Show the sheet's images beside the breakdown", scope: 'menu' },
  { id: 'view.single', label: 'Back to a single view', keys: 'mod+shift+0', scope: 'app' },

  // The Board menu. Cmd B is the reference app's New Banner, which goes above the selected
  // strip, the bottommost of several (behavior notes 17 and 27); the other keys are
  // provisional.
  { id: 'board.banner_above', label: 'Insert a banner above the selection', keys: 'mod+b', scope: 'app' },
  { id: 'board.banner_below', label: 'Insert a banner below the selection', keys: 'mod+shift+b', scope: 'app' },
  // Insert Day Break: the reference app's older manuals give Cmd comma (inventory, section
  // 14), but Cmd comma is the Mac's Settings key, so ours is mod+d (decisions.md, entry
  // 78); below the selection, as the B key. Above moved off mod+shift+d, the reference
  // app's Calendar key (decisions.md, entry 106).
  { id: 'board.break_below', label: 'Insert a day break below the selection', keys: 'mod+d', scope: 'app' },
  { id: 'board.break_above', label: 'Insert a day break above the selection', keys: 'mod+alt+d', scope: 'app' },
  { id: 'board.continue_scene', label: 'Continue the selected scene on the next day (a scene spanning days)', scope: 'menu' },
  { id: 'board.remove_breaks', label: "Remove every day break on the selection's sub-board", scope: 'menu' },
  { id: 'board.edit_banner', label: 'Edit the selected banner (or double-click it)', scope: 'menu' },
  { id: 'board.duplicate', label: 'Duplicate the selected scenes, each copy right after its strip', scope: 'menu' },
  { id: 'board.merge', label: 'Merge the selected scenes into one sheet', scope: 'menu' },
  { id: 'board.sort', label: 'Sort the strips of a sub-board or the Boneyard', scope: 'menu' },
  { id: 'board.layout', label: 'Choose the fields the strips show, their order, widths and lines, and the layout the board uses', scope: 'menu' },
  { id: 'board.layouts', label: 'Rename, reorder and delete the named layouts, and save or load them as files', scope: 'menu' },
  { id: 'board.print', label: 'Print the board shown', scope: 'menu' },
  { id: 'board.save_pdf', label: 'Save the board shown as a PDF', scope: 'menu' },
  { id: 'board.day_look', label: 'Choose what day breaks show, and a shoot day strip above each', scope: 'menu' },
  { id: 'board.show', label: 'Show or hide day headings, day breaks, their totals and banners on the board', scope: 'menu' },
  { id: 'board.colors', label: 'Set strip colors by I/E and time of day, and text colors', scope: 'menu' },
  { id: 'boards.manage', label: 'Add, rename, reorder and delete boards and sub-boards', scope: 'menu' },
  { id: 'boards.new_board', label: 'Add a board', scope: 'menu' },
  { id: 'boards.duplicate', label: 'Duplicate the board shown', scope: 'menu' },
  { id: 'boards.new_sub', label: 'Add a sub-board to the board shown', scope: 'menu' },

  // The Breakdown menu
  { id: 'breakdown.new_sheet', label: 'Add a sheet after the one shown', scope: 'menu' },
  { id: 'breakdown.duplicate_sheet', label: 'Duplicate the sheet shown', scope: 'menu' },
  { id: 'breakdown.delete_sheet', label: 'Delete the sheet shown', scope: 'menu' },
  { id: 'breakdown.go_scene', label: 'Go to a scene by its number', keys: 'mod+j', scope: 'app' },
  { id: 'breakdown.go_sheet', label: 'Go to a sheet by its sheet number', scope: 'menu' },
  { id: 'breakdown.field_order', label: "Choose the order of the sheet's header fields", scope: 'menu' },

  // The Plan menu (plan.js)
  { id: 'plan.import_tags', label: 'Import a tagged .fdx script with its production tags as elements', scope: 'menu' },
  { id: 'plan.revise', label: 'Merge a revised script into this schedule, keeping tags and places', scope: 'menu' },
  { id: 'plan.timeline', label: "Each shoot day's clock times from its call time, with warnings", scope: 'menu' },
  { id: 'plan.locations', label: 'Place sheet locations on the map for sunrise and sunset', scope: 'menu' },
  { id: 'plan.rest', label: 'Set turnaround minimums and list rests shorter than them', scope: 'menu' },
  { id: 'plan.print_call_sheet', label: "Print a shoot day's call sheet, or save it as a PDF", scope: 'menu' },
  { id: 'plan.call_sheet', label: "Export a shoot day's call sheet data as JSON or CSV", scope: 'menu' },
  { id: 'plan.locks', label: "Lock the schedule and see each actor's drift since", scope: 'menu' },
  { id: 'plan.pin', label: 'Keep the selected scenes in place when suggesting an order', scope: 'menu' },
  { id: 'plan.unpin', label: 'Let the selected scenes move when suggesting an order', scope: 'menu' },
  { id: 'plan.suggest', label: "Suggest an order for the selection's sub-board, with before and after counts", scope: 'menu' },

  // The board (click, Shift-click and Command-click or Ctrl-click select; drag to move)
  { id: 'board.previous', label: 'Select the strip above', keys: 'up', scope: 'board' },
  { id: 'board.next', label: 'Select the strip below', keys: 'down', scope: 'board' },
  { id: 'board.extend_up', label: 'Add the strip above to the selection', keys: 'shift+up', scope: 'board' },
  { id: 'board.extend_down', label: 'Add the strip below to the selection', keys: 'shift+down', scope: 'board' },
  { id: 'board.move_up', label: 'Move the selected strips up one place', keys: 'alt+up', scope: 'board' },
  { id: 'board.move_down', label: 'Move the selected strips down one place', keys: 'alt+down', scope: 'board' },
  { id: 'board.insert_break', label: 'Insert a day break below the selected strip', keys: 'b', scope: 'board' },
  { id: 'board.delete_break', label: 'Delete the selected strips (a scene deletes its sheet everywhere, after asking)', keys: ['backspace', 'delete'], scope: 'board' },
  { id: 'board.open_sheet', label: "Open the selected scene's breakdown sheet or banner (or double-click it)", keys: 'enter', scope: 'board' },

  // The Breakdown tab
  { id: 'sheet.previous', label: 'Previous sheet (in the order picked beside Previous and Next)', keys: 'mod+[', scope: 'sheet' },
  { id: 'sheet.next', label: 'Next sheet (in the order picked beside Previous and Next)', keys: 'mod+]', scope: 'sheet' },
  { id: 'sheet.first', label: 'First sheet', keys: 'mod+shift+[', scope: 'sheet' },
  { id: 'sheet.last', label: 'Last sheet', keys: 'mod+shift+]', scope: 'sheet' },

  // Help
  { id: 'help.guide', label: 'Read the user guide', scope: 'menu' },
  { id: 'help.shortcuts', label: 'List every command and its keys', keys: 'mod+/', scope: 'app' },
  { id: 'help.whats_new', label: "Read this version's release notes", scope: 'menu' },
  { id: 'help.open_sample', label: 'Open the invented sample schedule, untitled', scope: 'menu' },
  { id: 'help.report_bug', label: 'Open a new bug report in the browser, with the version filled in', scope: 'menu' },
  { id: 'help.licenses', label: 'Read the licenses and the third-party notices', scope: 'menu' },
  { id: 'help.suite', label: 'Open methodproduction.com, the Method Production Suite', scope: 'menu' },
  { id: 'help.check_updates', label: 'Check now for a newer version (macOS)', scope: 'menu' },
  { id: 'help.auto_updates', label: 'Check for a newer version now and then, on its own (on unless turned off)', scope: 'menu' },

  // Keys a pane answers itself (listed for the shortcut sheet)
  { id: 'page.strip_menu', label: "The strips' context menu, on the selection", mac: 'shift+f10', other: ['shift+f10', 'contextmenu'], scope: 'page' },
  { id: 'page.find_step', label: 'In the Find box: the next result (Shift: the previous one)', keys: 'enter', scope: 'page' },
  { id: 'page.close_panel', label: 'Close the Find panel, a dialog or a menu', keys: 'escape', scope: 'page' },
  { id: 'page.day', label: "In the Calendar's month: the day before or after", keys: ['left', 'right'], scope: 'page' },
  { id: 'page.week', label: "In the Calendar's month: the week before or after", keys: ['up', 'down'], scope: 'page' },
  { id: 'page.month', label: "In the Calendar's month: the month before or after", keys: ['pageup', 'pagedown'], scope: 'page' },
  { id: 'page.dood_row', label: 'In the Day out of days grid: the row above or below; Enter explains it', keys: ['up', 'down', 'enter'], scope: 'page' },
  { id: 'page.split_size', label: 'On the split bar: make the pane beside the board wider or narrower', keys: ['left', 'right'], scope: 'page' },
];

/** On macOS the menu runs app keys (see src/menu.rs); main.js leaves them alone there. */
export const menuRunsAppKeys = isMac;

const byId = new Map(SHORTCUTS.map((s) => [s.id, s]));

export function shortcutById(id) {
  return byId.get(id) || null;
}

/** The key specs for this platform, first one first. */
export function keysFor(entry) {
  if (!entry) return [];
  const k = entry.keys !== undefined ? entry.keys : isMac ? entry.mac : entry.other;
  if (k === undefined || k === null) return [];
  return Array.isArray(k) ? k : [k];
}

const MAC_NAMES = { backspace: '⌫', delete: '⌦', up: '↑', down: '↓', left: '←', right: '→', enter: '↩', escape: 'Esc', pageup: 'Page Up', pagedown: 'Page Down', f10: 'F10', contextmenu: 'Menu key' };
const PC_NAMES = { backspace: 'Backspace', delete: 'Delete', up: 'Up', down: 'Down', left: 'Left', right: 'Right', enter: 'Enter', escape: 'Escape', pageup: 'Page Up', pagedown: 'Page Down', f10: 'F10', contextmenu: 'Menu key' };

function parse(spec) {
  const parts = spec.toLowerCase().split('+');
  const key = parts.pop();
  return { key, mod: parts.includes('mod'), shift: parts.includes('shift'), alt: parts.includes('alt') };
}

/** One key spec for display: "⇧⌘S" on a Mac, "Ctrl+Shift+S" elsewhere. */
export function keyText(spec) {
  const p = parse(spec);
  if (isMac) {
    const k = MAC_NAMES[p.key] || p.key.toUpperCase();
    return (p.alt ? '⌥' : '') + (p.shift ? '⇧' : '') + (p.mod ? '⌘' : '') + k;
  }
  const k = PC_NAMES[p.key] || p.key.toUpperCase();
  const mods = [];
  if (p.mod) mods.push('Ctrl');
  if (p.alt) mods.push('Alt');
  if (p.shift) mods.push('Shift');
  return [...mods, k].join('+');
}

/** "Save (⌘S)", or just the label when there's no shortcut. */
export function withShortcut(label, id) {
  const k = keysFor(byId.get(id));
  return k.length ? `${label} (${keyText(k[0])})` : label;
}

const NAMED = { up: 'ArrowUp', down: 'ArrowDown', backspace: 'Backspace', delete: 'Delete', enter: 'Enter' };

/** Punctuation by physical key, for when Shift turns it into another character. */
const CODES = { ',': 'Comma', '.': 'Period', '[': 'BracketLeft', ']': 'BracketRight', '/': 'Slash' };

function keyMatches(e, k) {
  if (NAMED[k]) return e.key === NAMED[k];
  if (CODES[k]) return e.key === k || e.code === CODES[k];
  // Digits by physical key too: Shift turns 2 into @ on some layouts.
  if (/^[0-9]$/.test(k)) return e.key === k || e.code === `Digit${k}`;
  const key = typeof e.key === 'string' ? e.key : '';
  if (/^[a-z]$/.test(k)) {
    // Trust the layout's letter; Option on a Mac turns letters into symbols, so fall back
    // to the physical key then.
    if (/^[a-z]$/i.test(key)) return key.toLowerCase() === k;
    return e.code === `Key${k.toUpperCase()}`;
  }
  return key === k;
}

/** True when a key event is exactly this spec (no extra modifiers). */
export function matches(e, spec) {
  const p = parse(spec);
  if (isMac) {
    if (e.metaKey !== p.mod || e.ctrlKey) return false;
  } else if (e.metaKey || e.ctrlKey !== p.mod) {
    return false;
  }
  if (e.shiftKey !== p.shift || e.altKey !== p.alt) return false;
  if (typeof e.getModifierState === 'function' && e.getModifierState('AltGraph')) return false;
  return keyMatches(e, p.key);
}

/** The `app`, `board` or `sheet` command a key event triggers, or null. */
export function matchShortcut(e) {
  if (e.isComposing) return null;
  for (const s of SHORTCUTS) {
    if (s.scope === 'menu' || s.scope === 'page') continue;
    if (keysFor(s).some((k) => matches(e, k))) return s;
  }
  return null;
}
