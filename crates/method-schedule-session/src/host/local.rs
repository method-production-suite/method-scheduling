//! The desktop commands whose state the desktop keeps beside the schedule (the
//! preferences, the clipboard), kept by the host here: the preferences for every schedule
//! the host has open, a clipboard in each schedule.

use serde::Deserialize;
use serde_json::Value;

use super::{args, finish, reply, DocState, Error, Host, W};
use crate::clipboard;
use crate::reportsetup::Prefs;

/// The commands here.
pub(super) const COMMANDS: &[&str] = &[
    "preferences",
    "set_preferences",
    "copy_strips",
    "copy_elements",
];

/// All of them: none changes a schedule (a cut moves nothing until the paste).
pub(super) const READS: &[&str] = COMMANDS;

#[derive(Deserialize)]
struct PrefsArg {
    prefs: Prefs,
}

#[derive(Deserialize)]
struct CopyStrips {
    board: u32,
    strips: Vec<u32>,
    cut: bool,
}

#[derive(Deserialize)]
struct CopyElements {
    sheet: u32,
    elements: Vec<u32>,
    cut: bool,
}

impl Host {
    /// The preferences the host's schedules are shown with (the desktop's Settings).
    pub fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    /// Runs one of [`COMMANDS`], or `None` for any other command.
    pub(super) fn run_local(
        &mut self,
        command: &str,
        a: &Value,
        state: Option<DocState>,
    ) -> Option<Result<Value, Error>> {
        if !COMMANDS.contains(&command) {
            return None;
        }
        let need = || {
            state
                .clone()
                .ok_or_else(|| Error::Refused("No schedule is open.".into()))
        };
        Some((|| -> Result<Value, Error> {
            match command {
                "preferences" => reply(Ok(self.prefs.clone())),
                "set_preferences" => {
                    let p: PrefsArg = args(command, a)?;
                    self.prefs = p.prefs.checked()?;
                    reply(Ok(self.prefs.clone()))
                }
                "copy_strips" => {
                    let c: CopyStrips = args(command, a)?;
                    let state = need()?;
                    let mut s = state.lock();
                    let clip = clipboard::copy_strips(s.schedule(), c.board, &c.strips, c.cut)?;
                    let n = clipboard::count_text(c.strips.len(), "strip", "strips");
                    let notice = if c.cut {
                        format!("Cut {n}. Paste moves them below the selection on this board.")
                    } else {
                        format!("Copied {n}. Paste puts copies below the selection.")
                    };
                    s.set_clipboard(Some(clip));
                    reply(finish(&W, &s, Some(notice)))
                }
                "copy_elements" => {
                    let c: CopyElements = args(command, a)?;
                    let state = need()?;
                    let mut s = state.lock();
                    let clip = clipboard::copy_elements(s.schedule(), c.sheet, &c.elements, c.cut)?;
                    let n = clipboard::count_text(c.elements.len(), "element", "elements");
                    let notice = if c.cut {
                        format!("Cut {n}. Paste on another sheet moves them there.")
                    } else {
                        format!("Copied {n}. Paste puts them on the sheet shown.")
                    };
                    s.set_clipboard(Some(clip));
                    reply(finish(&W, &s, Some(notice)))
                }
                _ => Err(Error::UnknownCommand(command.to_string())),
            }
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Host;
    use serde_json::{json, Value};

    #[test]
    fn preferences_are_the_hosts_and_checked() {
        let mut h = Host::new();
        let p = h.dispatch("preferences", &Value::Null).unwrap();
        assert_eq!(p["fountain_lines"], 56);
        let mut q = p.clone();
        q["week_start"] = json!("mon");
        let back = h
            .dispatch("set_preferences", &json!({ "prefs": q }))
            .unwrap();
        assert_eq!(back["week_start"], "mon");
        assert_eq!(h.prefs().week_start, "mon");
        q["week_start"] = json!("tue");
        assert!(h
            .dispatch("set_preferences", &json!({ "prefs": q }))
            .is_err());
        assert_eq!(
            h.prefs().week_start,
            "mon",
            "a refused change changes nothing"
        );
    }

    #[test]
    fn strips_copy_and_paste_in_one_schedule() {
        let mut h = Host::new();
        let v = h.dispatch("open_sample", &Value::Null).unwrap()["view"].clone();
        let board = v["board"]["id"].as_u64().unwrap();
        let sub = v["board"]["sub_boards"][0]["id"].as_u64().unwrap();
        let strip = v["board"]["sub_boards"][0]["days"][0]["strips"][0]["id"]
            .as_u64()
            .unwrap();
        let c = h
            .dispatch(
                "copy_strips",
                &json!({ "board": board, "strips": [strip], "cut": false }),
            )
            .unwrap();
        assert!(c["notice"].as_str().unwrap().starts_with("Copied 1 strip"));
        h.dispatch(
            "paste_strips",
            &json!({ "board": board, "sub_board": sub, "index": 0 }),
        )
        .unwrap();
    }
}
