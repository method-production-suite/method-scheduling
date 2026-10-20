//! The native menu bar, from one table.
//!
//! Every command item carries an id and keys from the page's shortcut list,
//! `ui/js/shortcuts.js` (a test below fails if the two drift apart). Choosing one sends its
//! id to the page as a [`EVENT`] event, and `main.js` runs the same handler its key press
//! runs, so a menu click and a key always do the same thing. [`QUIT`] is answered in Rust
//! instead (it asks about unsaved changes first, `app.rs`).
//!
//! Who answers a key: on macOS the page sees a key first and the menu's key equivalent
//! fires only if the page didn't handle it, so `main.js` leaves app keys (Save, Undo) to
//! the menu there. On Windows and Linux `main.js` handles them itself and drops a menu
//! event that repeats a key it just handled. Method Budget's `src/menu.rs`, scaled down.

use tauri::menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub use method_schedule_session::menu::*;

/// A shortcut-list key spec as a Tauri accelerator: `mod+shift+s` is `CmdOrCtrl+Shift+S`.
pub fn accelerator(spec: &str) -> String {
    spec.split('+')
        .map(|part| match part {
            "mod" => "CmdOrCtrl".to_string(),
            "ctrl" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            key => key.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn append<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Submenu<R>,
    entries: &[Entry],
) -> tauri::Result<()> {
    for entry in entries {
        match *entry {
            Entry::Item {
                id,
                label,
                mac,
                other,
            } => {
                let keys = if cfg!(target_os = "macos") {
                    mac
                } else {
                    other
                };
                let accel = keys.map(accelerator);
                menu.append(&MenuItem::with_id(app, id, label, true, accel.as_deref())?)?;
            }
            Entry::Check { id, label } => {
                // On until the settings say otherwise; src/safety_app.rs (Autosave) and
                // src/updater.rs (automatic update checks) keep each in step.
                let on = if id == crate::updater::AUTO_ITEM {
                    app.try_state::<crate::updater::Updates>()
                        .is_none_or(|u| u.automatic())
                } else {
                    app.try_state::<crate::safety_app::Safety>()
                        .is_none_or(|s| s.autosave_on())
                };
                menu.append(&CheckMenuItem::with_id(
                    app,
                    id,
                    label,
                    true,
                    on,
                    None::<&str>,
                )?)?;
            }
            Entry::OpenRecent => {
                let recent = Submenu::with_id(app, OPEN_RECENT, "Open Recent", true)?;
                recent.append(&MenuItem::with_id(
                    app,
                    CLEAR_RECENT,
                    "Clear Menu",
                    false,
                    None::<&str>,
                )?)?;
                menu.append(&recent)?;
            }
            Entry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
            Entry::Cut => menu.append(&PredefinedMenuItem::cut(app, None)?)?,
            Entry::Copy => menu.append(&PredefinedMenuItem::copy(app, None)?)?,
            Entry::Paste => menu.append(&PredefinedMenuItem::paste(app, None)?)?,
            Entry::SelectAll => menu.append(&PredefinedMenuItem::select_all(app, None)?)?,
        }
    }
    Ok(())
}

fn submenu<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    entries: &[Entry],
) -> tauri::Result<Submenu<R>> {
    let menu = Submenu::new(app, title, true)?;
    append(app, &menu, entries)?;
    Ok(menu)
}

/// About Method Schedule: the version, the website and the licence (decision 212).
fn about<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PredefinedMenuItem<R>> {
    let name = app.package_info().name.clone();
    PredefinedMenuItem::about(
        app,
        Some(&format!("About {name}")),
        Some(AboutMetadata {
            name: Some(name.clone()),
            version: Some(app.package_info().version.to_string()),
            // A link on Windows and Linux; macOS shows the credits line.
            website: Some(crate::help::WEBSITE.into()),
            website_label: Some("methodschedule.com".into()),
            credits: Some("methodschedule.com".into()),
            copyright: Some(
                "Copyright (c) 2026 Method Production Suite Team and the Method Schedule contributors"
                    .into(),
            ),
            license: Some("MIT OR Apache-2.0".into()),
            ..Default::default()
        }),
    )
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;

    #[cfg(target_os = "macos")]
    {
        // The app menu. Our own Quit item goes through the unsaved-changes guard; the
        // stock one would quit without asking.
        let name = app.package_info().name.clone();
        let app_menu = Submenu::new(app, &name, true)?;
        app_menu.append(&about(app)?)?;
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        append(app, &app_menu, SETTINGS)?;
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        app_menu.append(&PredefinedMenuItem::hide(
            app,
            Some(&format!("Hide {name}")),
        )?)?;
        app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        app_menu.append(&MenuItem::with_id(
            app,
            QUIT,
            format!("Quit {name}"),
            true,
            Some(accelerator("mod+q")),
        )?)?;
        menu.append(&app_menu)?;
    }

    let file = submenu(app, "&File", FILE)?;
    if !cfg!(target_os = "macos") {
        append(app, &file, FILE_QUIT)?;
    }
    menu.append(&file)?;
    let edit = submenu(app, "&Edit", EDIT)?;
    if !cfg!(target_os = "macos") {
        append(app, &edit, &[Entry::Separator])?;
        append(app, &edit, SETTINGS)?;
    }
    menu.append(&edit)?;
    menu.append(&submenu(app, "&View", VIEW)?)?;
    menu.append(&submenu(app, "&Board", BOARD)?)?;
    menu.append(&submenu(app, "B&reakdown", BREAKDOWN)?)?;
    menu.append(&submenu(app, "&Plan", PLAN)?)?;

    #[cfg(target_os = "macos")]
    {
        let window = Submenu::with_id(app, tauri::menu::WINDOW_SUBMENU_ID, "Window", true)?;
        window.append(&PredefinedMenuItem::minimize(app, None)?)?;
        window.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
        menu.append(&window)?;
    }
    // Every open window, to bring one to the front (src/windows.rs fills it in). macOS
    // lists them in its own Window menu.
    if !cfg!(target_os = "macos") {
        menu.append(&Submenu::with_id(
            app,
            crate::windows::WINDOW_LIST,
            "&Window",
            true,
        )?)?;
    }
    let help = Submenu::with_id(app, tauri::menu::HELP_SUBMENU_ID, "&Help", true)?;
    append(app, &help, HELP)?;
    if !cfg!(target_os = "macos") {
        help.append(&PredefinedMenuItem::separator(app)?)?;
        help.append(&about(app)?)?;
    }
    menu.append(&help)?;
    Ok(menu)
}

/// Sends a menu item's id to the page of the window in front (decisions.md, entry 86).
pub fn handle<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let label = crate::windows::focused(app)
        .map(|w| w.label().to_string())
        .unwrap_or_else(|| crate::app::MAIN.to_string());
    let _ = app.emit_to(label.as_str(), EVENT, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORTCUTS_JS: &str = include_str!("../ui/js/shortcuts.js");

    /// Every command item in the menus, check items as items without keys.
    fn items() -> Vec<Entry> {
        [
            FILE, FILE_QUIT, SETTINGS, EDIT, VIEW, BOARD, BREAKDOWN, PLAN, HELP,
        ]
        .iter()
        .flat_map(|m| m.iter().copied())
        .filter_map(|e| match e {
            Entry::Item { .. } => Some(e),
            Entry::Check { id, label } => Some(Entry::Item {
                id,
                label,
                mac: None,
                other: None,
            }),
            _ => None,
        })
        .collect()
    }

    /// The text after `key: '` (or `key: ['`) up to the next quote.
    fn quoted_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let at = line.find(&format!("{key}: "))? + key.len() + 2;
        let rest = line[at..].trim_start_matches('[');
        let rest = rest.strip_prefix('\'')?;
        Some(&rest[..rest.find('\'')?])
    }

    /// (mac keys, other keys, scope) for an id in shortcuts.js; the first alternative when
    /// there are several.
    fn js_entry(id: &str) -> Option<(Option<&str>, Option<&str>, &str)> {
        let needle = format!("{{ id: '{id}',");
        let line = SHORTCUTS_JS.lines().find(|l| l.contains(&needle))?;
        let scope = quoted_after(line, "scope").expect("every entry has a scope");
        if let Some(k) = quoted_after(line, "keys") {
            return Some((Some(k), Some(k), scope));
        }
        Some((
            quoted_after(line, "mac"),
            quoted_after(line, "other"),
            scope,
        ))
    }

    #[test]
    fn menu_keys_match_the_shortcut_list() {
        for e in items() {
            let Entry::Item { id, mac, other, .. } = e else {
                unreachable!()
            };
            let (js_mac, js_other, scope) =
                js_entry(id).unwrap_or_else(|| panic!("{id} is missing from shortcuts.js"));
            assert_eq!(mac, js_mac, "macOS keys for {id}");
            assert_eq!(other, js_other, "Windows/Linux keys for {id}");
            assert!(matches!(scope, "app" | "menu"), "{id} is {scope}");
        }
    }

    #[test]
    fn the_banner_key_inserts_above_first_in_the_menu() {
        // Behavior note 17: the reference app's New Banner (Cmd B) goes above the
        // selected strip.
        let first = BOARD.iter().find_map(|e| match e {
            Entry::Item { id, mac, .. } => Some((*id, *mac)),
            _ => None,
        });
        assert_eq!(first, Some(("board.banner_above", Some("mod+b"))));
    }

    #[test]
    fn every_app_shortcut_has_a_menu_item() {
        // App keys only work through the menu on macOS, so each one needs an item.
        let ids: Vec<&str> = items()
            .into_iter()
            .map(|e| match e {
                Entry::Item { id, .. } => id,
                _ => unreachable!(),
            })
            .collect();
        let mut checked = 0;
        for line in SHORTCUTS_JS.lines() {
            let Some(id) = quoted_after(line, "{ id") else {
                continue;
            };
            let scope = quoted_after(line, "scope").unwrap();
            if matches!(scope, "app" | "menu") {
                assert!(ids.contains(&id), "{id} ({scope}) has no menu item");
                checked += 1;
            }
        }
        assert_eq!(checked, ids.len(), "shortcuts.js lists every menu item");
    }

    #[test]
    fn no_two_items_share_keys_and_labels_follow_the_voice() {
        for pick in [0, 1] {
            let mut seen = std::collections::BTreeMap::new();
            for e in items() {
                let Entry::Item {
                    id,
                    label,
                    mac,
                    other,
                } = e
                else {
                    unreachable!()
                };
                assert!(
                    !label.contains('\u{2014}') && !label.contains('!'),
                    "{label}"
                );
                if let Some(k) = [mac, other][pick] {
                    if let Some(prev) = seen.insert(k, id) {
                        panic!("{prev} and {id} both use {k}");
                    }
                }
            }
        }
    }

    #[test]
    fn recent_ids_stay_clear_of_the_table() {
        // Recent files are numbered under their own prefix, which no table id uses.
        for e in items() {
            let Entry::Item { id, .. } = e else {
                unreachable!()
            };
            assert!(!id.starts_with(RECENT_PREFIX), "{id}");
        }
        assert!(CLEAR_RECENT.starts_with(RECENT_PREFIX));
        assert!(FILE.contains(&Entry::OpenRecent));
    }

    #[test]
    fn accelerators() {
        assert_eq!(accelerator("mod+shift+s"), "CmdOrCtrl+Shift+S");
        assert_eq!(accelerator("mod+z"), "CmdOrCtrl+Z");
        assert_eq!(accelerator("mod+."), "CmdOrCtrl+.");
        assert_eq!(accelerator("mod+alt+d"), "CmdOrCtrl+Alt+D");
    }

    #[test]
    fn every_accelerator_names_a_key_the_menu_takes() {
        // The keys the native menu's accelerator parser reads (letters, digits and these
        // marks); anything else would fail when the menu is built.
        for e in items() {
            let Entry::Item { id, mac, other, .. } = e else {
                unreachable!()
            };
            for spec in [mac, other].into_iter().flatten() {
                let key = spec.rsplit('+').next().unwrap();
                let ok = (key.len() == 1
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b",./[]".contains(&b)))
                    || matches!(key, "up" | "down" | "enter" | "backspace" | "delete");
                assert!(ok, "{id}: {spec}");
            }
        }
    }

    #[test]
    fn the_reference_keys_are_here() {
        // The keys the reference app documents, where its command exists here (inventory
        // section 14, behavior note 21): New, Open, Save, Save As, Close, Undo, Redo, the
        // banner and the tabs it has keys for.
        let keys: std::collections::BTreeMap<&str, &str> = items()
            .into_iter()
            .filter_map(|e| match e {
                Entry::Item { id, mac, .. } => Some((id, mac?)),
                _ => None,
            })
            .collect();
        for (id, k) in [
            ("file.new", "mod+n"),
            ("file.open", "mod+o"),
            ("file.save", "mod+s"),
            ("file.save_as", "mod+shift+s"),
            (CLOSE, "mod+w"),
            ("edit.undo", "mod+z"),
            ("edit.redo", "mod+shift+z"),
            ("board.banner_above", "mod+b"),
            ("board.break_below", "mod+d"),
            ("app.settings", "mod+,"),
            ("breakdown.go_scene", "mod+j"),
            ("edit.links", "mod+l"),
            ("edit.add_red_flag", "mod+k"),
            ("file.print", "mod+p"),
            ("edit.find", "mod+f"),
            // Behavior note 21 (decisions.md, entry 106): the Schedule menu's keys where
            // they don't clash with a platform convention. Its Reports key, mod+p, is
            // Print (above), so Reports keeps ours.
            ("view.board", "mod+u"),
            ("view.breakdown", "mod+."),
            ("view.calendar", "mod+shift+d"),
            ("view.reports", "mod+6"),
            ("board.break_above", "mod+alt+d"),
        ] {
            assert_eq!(keys.get(id), Some(&k), "{id}");
        }
    }
}
