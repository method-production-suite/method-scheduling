//! The menu bar's table, without a window: every menu, its items' ids, labels and keys.
//! The desktop builds its native menu from it (method-schedule-app's menu.rs); a browser
//! host draws its own menu bar from [`web_menus`]. Choosing an item sends its id to the
//! page, which runs the same handler as the item's key.

use serde::Serialize;

/// Help > Check for Updates (the desktop's updater).
pub const UPDATE_CHECK: &str = "help.check_updates";
/// Help > Check for Updates Automatically, a check item.
pub const UPDATE_AUTO: &str = "help.auto_updates";

/// The event the page listens for; the payload is the item's id.
pub const EVENT: &str = "menu";
/// Quit, after asking about unsaved changes (`app.rs`).
pub const QUIT: &str = "app.quit";
/// File > Open Recent, a submenu filled from the recent list (src/safety_app.rs).
pub const OPEN_RECENT: &str = "file.open_recent";
/// File > Open Recent > one file: `file.recent.<n>`, answered in the page (which runs
/// `open_recent` with `n`).
pub const RECENT_PREFIX: &str = "file.recent.";
/// File > Open Recent > Clear Menu, answered in Rust.
pub const CLEAR_RECENT: &str = "file.recent.clear";
/// File > Autosave, a check item answered in Rust.
pub const AUTOSAVE: &str = "file.autosave";
/// File > Close Window (mod+w, the reference app's Close): answered in Rust, which closes
/// the focused window through the unsaved-changes question (decisions.md, entry 79).
pub const CLOSE: &str = "file.close";

/// One menu entry. Keys use the shortcut list's syntax (`mod+shift+s`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Item {
        id: &'static str,
        label: &'static str,
        mac: Option<&'static str>,
        other: Option<&'static str>,
    },
    /// A check item answered in Rust (src/safety_app.rs).
    Check {
        id: &'static str,
        label: &'static str,
    },
    /// File > Open Recent: a submenu whose items src/safety_app.rs fills in.
    OpenRecent,
    Separator,
    /// Stock items: they act on text fields natively.
    Cut,
    Copy,
    Paste,
    SelectAll,
}

use Entry::Separator as SEP;

const fn item(id: &'static str, label: &'static str, keys: &'static str) -> Entry {
    Entry::Item {
        id,
        label,
        mac: Some(keys),
        other: Some(keys),
    }
}

const fn per_platform(
    id: &'static str,
    label: &'static str,
    mac: &'static str,
    other: &'static str,
) -> Entry {
    Entry::Item {
        id,
        label,
        mac: Some(mac),
        other: Some(other),
    }
}

const fn plain(id: &'static str, label: &'static str) -> Entry {
    Entry::Item {
        id,
        label,
        mac: None,
        other: None,
    }
}

pub const FILE: &[Entry] = &[
    item("file.new", "New", "mod+n"),
    item("file.new_window", "New Window", "mod+shift+n"),
    item("file.open", "Open\u{2026}", "mod+o"),
    plain("file.open_window", "Open in New Window\u{2026}"),
    Entry::OpenRecent,
    plain("file.new_from_template", "New from Template\u{2026}"),
    SEP,
    item("file.save", "Save", "mod+s"),
    item("file.save_as", "Save As\u{2026}", "mod+shift+s"),
    Entry::Check {
        id: AUTOSAVE,
        label: "Autosave",
    },
    plain("file.save_template", "Save as Template\u{2026}"),
    plain("file.revert_backup", "Revert to Backup\u{2026}"),
    SEP,
    plain("file.import_script", "Import Script\u{2026}"),
    SEP,
    plain("file.what_changed", "What Changed\u{2026}"),
    plain("file.what_changed_saved", "What Changed Since Saved"),
    plain("file.merge", "Merge Changes From\u{2026}"),
    SEP,
    item("file.export", "Export\u{2026}", "mod+e"),
    plain("file.export_budget", "Export for Budget\u{2026}"),
    SEP,
    item("file.print", "Print\u{2026}", "mod+p"),
    plain("file.save_pdf", "Save as PDF\u{2026}"),
    SEP,
    item(CLOSE, "Close Window", "mod+w"),
];

/// Windows and Linux end the File menu with Quit; macOS has it in the app menu.
pub const FILE_QUIT: &[Entry] = &[SEP, item(QUIT, "Quit", "mod+q")];

/// Settings (mod+, as on every Mac app): in the app menu on macOS, at the end of the Edit
/// menu elsewhere (decisions.md, entry 79). The page answers it.
pub const SETTINGS: &[Entry] = &[item("app.settings", "Settings\u{2026}", "mod+,")];

pub const EDIT: &[Entry] = &[
    item("edit.undo", "Undo", "mod+z"),
    per_platform("edit.redo", "Redo", "mod+shift+z", "mod+y"),
    SEP,
    Entry::Cut,
    Entry::Copy,
    Entry::Paste,
    Entry::SelectAll,
    SEP,
    // Provisional keys (decisions.md, 2, 26 and 34).
    item("edit.find", "Find\u{2026}", "mod+f"),
    item("edit.replace", "Replace\u{2026}", "mod+shift+f"),
    item("edit.find_next", "Find Next", "mod+g"),
    item("edit.find_previous", "Find Previous", "mod+shift+g"),
    SEP,
    item("edit.explain", "Explain This Count", "mod+i"),
    SEP,
    // The older reference app's element linking and red flag entry keys (note 10).
    item("edit.links", "Linked Elements\u{2026}", "mod+l"),
    item("edit.add_red_flag", "Add Red Flag\u{2026}", "mod+k"),
    plain("edit.red_flag_types", "Red Flag Types\u{2026}"),
];

/// The tabs. Keys provisional until checked against the reference app (decisions.md, 2).
pub const VIEW: &[Entry] = &[
    // The reference app's keys for Board, Breakdown and Calendar (behavior note 21); its
    // Reports key is Print here (decisions.md, entry 106).
    item("view.board", "Board", "mod+u"),
    item("view.breakdown", "Breakdown", "mod+."),
    item("view.elements", "Elements", "mod+3"),
    item("view.dood", "Day out of days", "mod+4"),
    item("view.calendar", "Calendar", "mod+shift+d"),
    item("view.reports", "Reports", "mod+6"),
    item("view.conflicts", "Conflicts", "mod+7"),
    item("view.red_flags", "Red Flags", "mod+8"),
    SEP,
    // Split screen (decisions.md, entry 80): a pane beside the board.
    item(
        "view.split_breakdown",
        "Breakdown Beside Board",
        "mod+shift+2",
    ),
    item("view.split_board", "Board Beside Board", "mod+shift+1"),
    item(
        "view.split_calendar",
        "Calendar Beside Board",
        "mod+shift+5",
    ),
    plain("view.split_images", "Images Beside Board"),
    // The sheet's images beside the Breakdown tab (behavior note 22).
    plain("view.split_breakdown_images", "Images Beside Breakdown"),
    item("view.single", "Single View", "mod+shift+0"),
];

/// The Board menu: banners, duplicating and merging strips, sorting, boards and
/// sub-boards (decisions.md, 29 to 32, 37 and 44). The reference app's manual names Cmd B
/// for a new banner, and its New Banner goes above the selected strip (behavior note 17),
/// so Cmd B inserts above; the other keys are provisional.
pub const BOARD: &[Entry] = &[
    item("board.banner_above", "Insert Banner Above\u{2026}", "mod+b"),
    item(
        "board.banner_below",
        "Insert Banner Below\u{2026}",
        "mod+shift+b",
    ),
    plain("board.edit_banner", "Edit Banner\u{2026}"),
    SEP,
    // Cmd comma inserted a day break in the reference app's older versions (inventory,
    // section 14), but it is the Mac's Settings key: ours is mod+d (decisions.md,
    // entry 79). Provisional.
    item("board.break_below", "Insert Day Break Below", "mod+d"),
    item("board.break_above", "Insert Day Break Above", "mod+alt+d"),
    plain("board.remove_breaks", "Remove All Day Breaks"),
    plain("board.continue_scene", "Continue Scene on Next Day"),
    SEP,
    plain("board.duplicate", "Duplicate Strip"),
    plain("board.merge", "Merge Strips\u{2026}"),
    SEP,
    plain("board.sort", "Sort Strips\u{2026}"),
    plain("board.layout", "Strip Layout\u{2026}"),
    plain("board.layouts", "Layouts\u{2026}"),
    plain("board.colors", "Strip Colors\u{2026}"),
    plain("board.day_look", "Day Breaks\u{2026}"),
    plain("board.show", "Show on Board\u{2026}"),
    SEP,
    plain("board.print", "Print Board\u{2026}"),
    plain("board.save_pdf", "Save Board as PDF\u{2026}"),
    SEP,
    plain("boards.new_board", "New Board\u{2026}"),
    plain("boards.duplicate", "Duplicate Board"),
    plain("boards.new_sub", "New Sub-board\u{2026}"),
    plain("boards.manage", "Boards and Sub-boards\u{2026}"),
];

/// The Breakdown menu: the sheet shown on the Breakdown tab (decisions.md, 37). Its
/// commands show the Breakdown tab first.
pub const BREAKDOWN: &[Entry] = &[
    plain("breakdown.new_sheet", "New Sheet"),
    plain("breakdown.duplicate_sheet", "Duplicate Sheet"),
    SEP,
    plain("breakdown.delete_sheet", "Delete Sheet\u{2026}"),
    SEP,
    // The older reference app's find scene key (note 10); its find sheet key (mod+g) is
    // Find Next here.
    item("breakdown.go_scene", "Go to Scene\u{2026}", "mod+j"),
    plain("breakdown.go_sheet", "Go to Sheet Number\u{2026}"),
    SEP,
    plain("breakdown.field_order", "Breakdown Designer\u{2026}"),
];

/// The Help menu: the shortcut sheet, made from ui/js/shortcuts.js (decisions.md,
/// entry 79), the user guide and the sample schedule (entries 198 and 199, src/help.rs),
/// and the suite's items (entry 212): What's New, Report a Bug, Licenses and Notices and
/// the suite's website. On Windows and Linux About follows them.
pub const HELP: &[Entry] = &[
    plain("help.guide", "User Guide"),
    item("help.shortcuts", "Keyboard Shortcuts", "mod+/"),
    plain("help.whats_new", "What\u{2019}s New"),
    SEP,
    plain("help.open_sample", "Open Sample"),
    SEP,
    plain("help.report_bug", "Report a Bug\u{2026}"),
    plain("help.licenses", "Licenses and Notices"),
    plain("help.suite", "methodproduction.com"),
    SEP,
    plain(UPDATE_CHECK, "Check for Updates\u{2026}"),
    Entry::Check {
        id: UPDATE_AUTO,
        label: "Check for Updates Automatically",
    },
];

/// Builds the menu bar. `&` marks the Alt-key letter on Windows and Linux (macOS ignores
/// it).
/// The Plan menu: the features beyond the reference app (src/plan.rs; decisions.md,
/// IMP-04 and BYD-01 to BYD-09). No keys.
pub const PLAN: &[Entry] = &[
    plain("plan.import_tags", "Import Script with Tags\u{2026}"),
    plain("plan.revise", "Import Script Revision\u{2026}"),
    SEP,
    plain("plan.timeline", "Day Timeline\u{2026}"),
    plain("plan.locations", "Locations\u{2026}"),
    plain("plan.rest", "Turnarounds and Rest\u{2026}"),
    plain("plan.print_call_sheet", "Print Call Sheet\u{2026}"),
    plain("plan.call_sheet", "Export Call Sheet Data\u{2026}"),
    SEP,
    plain("plan.locks", "Schedule Locks\u{2026}"),
    SEP,
    plain("plan.pin", "Pin Strips"),
    plain("plan.unpin", "Unpin Strips"),
    plain("plan.suggest", "Suggest Strip Order\u{2026}"),
];

/// One menu as a browser host draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MenuDto {
    pub title: &'static str,
    /// Each item, `null` for a separator. The page shows each item's keys from its own
    /// shortcut list.
    pub items: Vec<Option<MenuItemDto>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MenuItemDto {
    pub id: &'static str,
    pub label: &'static str,
}

/// What only the desktop does, left out of the browser's menus: windows, autosave and
/// backups, merging (decision 226), saving a PDF (the browser's print dialog does), the
/// updater, and the items whose text is in the desktop app (the guide, What's New, the
/// licences, a bug report with the app's version).
pub const WEB_HIDDEN: &[&str] = &[
    "file.new_window",
    "file.open_window",
    AUTOSAVE,
    "file.revert_backup",
    "file.merge",
    "file.save_pdf",
    "board.save_pdf",
    CLOSE,
    QUIT,
    "help.guide",
    "help.whats_new",
    "help.licenses",
    "help.report_bug",
    UPDATE_CHECK,
    UPDATE_AUTO,
];

/// The menus a browser host draws: the desktop's, in its order, without [`WEB_HIDDEN`],
/// Open Recent or the stock text items (the browser has its own), and with no separator
/// first, last or twice.
pub fn web_menus() -> Vec<MenuDto> {
    let edit: Vec<Entry> = EDIT.iter().chain(&[SEP]).chain(SETTINGS).copied().collect();
    let menus: [(&'static str, &[Entry]); 7] = [
        ("File", FILE),
        ("Edit", &edit),
        ("View", VIEW),
        ("Board", BOARD),
        ("Breakdown", BREAKDOWN),
        ("Plan", PLAN),
        ("Help", HELP),
    ];
    menus
        .iter()
        .map(|(title, entries)| {
            let mut items: Vec<Option<MenuItemDto>> = Vec::new();
            for e in entries.iter() {
                match *e {
                    Entry::Item { id, label, .. } if !WEB_HIDDEN.contains(&id) => {
                        items.push(Some(MenuItemDto { id, label }))
                    }
                    Entry::Separator if items.last().is_some_and(Option::is_some) => {
                        items.push(None)
                    }
                    _ => {}
                }
            }
            while items.last().is_some_and(Option::is_none) {
                items.pop();
            }
            MenuDto { title, items }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_menus_leave_out_what_only_the_desktop_does() {
        let menus = web_menus();
        assert_eq!(menus.len(), 7);
        let ids: Vec<&str> = menus
            .iter()
            .flat_map(|m| m.items.iter().flatten().map(|i| i.id))
            .collect();
        for hidden in WEB_HIDDEN {
            assert!(!ids.contains(hidden), "{hidden}");
        }
        for shown in [
            "file.open",
            "file.save",
            "edit.undo",
            "app.settings",
            "plan.revise",
        ] {
            assert!(ids.contains(&shown), "{shown}");
        }
        for m in &menus {
            assert!(m.items.first().is_some_and(Option::is_some), "{}", m.title);
            assert!(m.items.last().is_some_and(Option::is_some), "{}", m.title);
            assert!(
                !m.items.windows(2).any(|w| w[0].is_none() && w[1].is_none()),
                "{}",
                m.title
            );
        }
    }
}
