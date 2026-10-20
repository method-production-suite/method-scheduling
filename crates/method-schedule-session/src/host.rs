//! The one entry point for a host that isn't the desktop app (a partner's web app, the
//! website's demo): [`Host::dispatch`] takes a command's name and its arguments as JSON,
//! exactly as the page's `invoke` sends them to the desktop, and gives back what the
//! desktop's command returns (docs/research/web-session.md, decision 222; Method Budget's
//! contract, so a partner integrates both apps the same way).
//!
//! The contract is versioned: [`COMMANDS_VERSION`] goes up whenever a command in
//! [`COMMANDS`] is removed or renamed, or its arguments or reply change in a way an existing
//! caller would notice. Adding a command or an optional argument doesn't change it.
//!
//! Schedules come and go as `.mtsc` text ([`Host::open_mtsc`], [`Host::save_mtsc`]): where
//! they are kept is the host's business. Each command takes `doc` (the schedule it acts on;
//! missing or `null`: the one in front) besides the desktop's arguments.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session::Session;
use crate::view::{self, View};

mod b64;
mod files;
mod generated;

pub use b64::{decode as b64_decode, encode as b64_encode};
pub use files::{Downloads, FileIn, FileOut};

/// The command contract's version (see the module's notes).
pub const COMMANDS_VERSION: u32 = 1;

/// The host's own commands, beside the desktop's ([`generated::COMMANDS`]).
const OWN: &[&str] = &[
    "open_mtsc",
    "save_mtsc",
    "close_doc",
    "list_docs",
    "host_info",
];

/// Every command [`Host::dispatch`] runs: the desktop's that need only the schedule, then
/// the host's own.
pub fn commands() -> Vec<&'static str> {
    generated::COMMANDS
        .iter()
        .chain(files::COMMANDS)
        .chain(OWN)
        .copied()
        .collect()
}

/// The desktop commands [`Host::dispatch`] runs that need only the schedule, by name
/// ([`commands`] adds those that take or give files, and the host's own).
pub const COMMANDS: &[&str] = generated::COMMANDS;

/// What an edit is told while the host shows schedules read-only ([`Host::set_read_only`]).
pub const READ_ONLY: &str = "This schedule is open read-only here, so it can't be changed.";

/// Why a command didn't run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No command of that name (or not one this host runs yet).
    UnknownCommand(String),
    /// The arguments didn't have the command's shape.
    BadArguments { command: String, message: String },
    /// The command was refused, with the message the page shows; nothing changed.
    Refused(String),
}

impl Error {
    /// The text the page shows: what the desktop's command would reject with.
    pub fn message(&self) -> String {
        match self {
            Error::UnknownCommand(c) => format!("Method Schedule has no command {c}."),
            Error::BadArguments { command, message } => {
                format!("The {command} command can't read its arguments: {message}")
            }
            Error::Refused(m) => m.clone(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(m: String) -> Error {
        Error::Refused(m)
    }
}

impl From<&str> for Error {
    fn from(m: &str) -> Error {
        Error::Refused(m.to_string())
    }
}

/// One open schedule, shared the way the desktop's window state is, so a command's body
/// reads the same here as there (`state.lock()`).
#[derive(Clone)]
pub struct DocState(Arc<Mutex<Session>>);

impl DocState {
    pub fn lock(&self) -> MutexGuard<'_, Session> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What a host is, for the page (the desktop answers `host_info` too).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HostInfo {
    /// `"web"` here; the desktop says `"desktop"`.
    pub kind: &'static str,
    pub version: &'static str,
    pub commands_version: u32,
    pub read_only: bool,
}

/// The reply to `open_mtsc`: the new schedule's id and its view.
#[derive(Clone, Debug, Serialize)]
pub struct Opened {
    pub doc: u32,
    pub view: View,
}

/// The reply to `save_mtsc`: the schedule's text, to keep wherever the host keeps it, and a
/// file name to offer it under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Saved {
    pub name: String,
    pub text: String,
}

/// Every open schedule, and the commands on them.
#[derive(Default)]
pub struct Host {
    docs: Vec<(u32, DocState)>,
    front: Option<u32>,
    next: u32,
    read_only: bool,
    /// The picture store: each picture's bytes by the name a schedule stores for it.
    images: HashMap<String, Vec<u8>>,
    /// Each schedule as it was last opened or saved from text, for What Changed's "since
    /// saved"; a new schedule has none.
    saved: HashMap<u32, method_schedule_core::Schedule>,
    /// The revision picked and previewed, waiting for its apply (the desktop keeps one too).
    revision: Option<files::Revision>,
    tokens: u64,
}

#[derive(Deserialize)]
struct DocArg {
    #[serde(default)]
    doc: Option<u32>,
}

#[derive(Deserialize)]
struct OpenArgs {
    text: String,
    #[serde(default)]
    name: Option<String>,
}

pub(crate) fn reply<T: Serialize>(v: Result<T, String>) -> Result<Value, Error> {
    let v = v.map_err(Error::Refused)?;
    serde_json::to_value(v).map_err(|e| Error::Refused(e.to_string()))
}

/// The arguments of `command` as its shape, or [`Error::BadArguments`].
pub(crate) fn args<T: DeserializeOwned>(command: &str, args: &Value) -> Result<T, Error> {
    let a = if args.is_null() {
        Value::Object(Default::default())
    } else {
        args.clone()
    };
    serde_json::from_value(a).map_err(|e| Error::BadArguments {
        command: command.to_string(),
        message: e.to_string(),
    })
}

impl Host {
    pub fn new() -> Host {
        Host::default()
    }

    /// Opens a schedule from its `.mtsc` text (the format's strict reader) and puts it in
    /// front. `name` is only shown, as the desktop shows a file's name.
    pub fn open_mtsc(&mut self, text: &str, name: Option<&str>) -> Result<Opened, Error> {
        let schedule = method_schedule_format::from_str(text).map_err(|e| {
            Error::Refused(format!(
                "This isn't a schedule Method Schedule can read: {e}"
            ))
        })?;
        let session = Session::with_schedule(schedule, name.map(std::path::PathBuf::from));
        self.add_saved_doc(session)
    }

    /// Puts a schedule opened from text in front, remembering it as saved.
    fn add_saved_doc(&mut self, session: Session) -> Result<Opened, Error> {
        let copy = session.schedule().clone();
        let opened = self.add_doc(session, None)?;
        self.saved.insert(opened.doc, copy);
        Ok(opened)
    }

    /// Puts `session` in front as a new schedule, with its view.
    fn add_doc(&mut self, session: Session, notice: Option<String>) -> Result<Opened, Error> {
        self.next += 1;
        let id = self.next;
        self.docs
            .push((id, DocState(Arc::new(Mutex::new(session)))));
        self.front = Some(id);
        let state = self.doc(Some(id))?;
        let view = finish(&W, &state.lock(), notice)?;
        Ok(Opened { doc: id, view })
    }

    /// The `.mtsc` text of schedule `doc` (the one in front when `None`). Nothing is
    /// written anywhere: keeping it is the host's.
    pub fn save_mtsc(&mut self, doc: Option<u32>) -> Result<Saved, Error> {
        let state = self.doc(doc)?;
        let s = state.lock();
        if let Some(id) = doc.or(self.front) {
            self.saved.insert(id, s.schedule().clone());
        }
        let name = format!("{}.mtsc", s.schedule().title.trim());
        Ok(Saved {
            name,
            text: s.file_text(),
        })
    }

    /// Shows every schedule read-only (a partner's viewer) or editable again. Read-only
    /// refuses every command that changes a schedule, with [`READ_ONLY`].
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
    }

    pub fn host_info(&self) -> HostInfo {
        HostInfo {
            kind: "web",
            version: env!("CARGO_PKG_VERSION"),
            commands_version: COMMANDS_VERSION,
            read_only: self.read_only,
        }
    }

    fn doc(&self, doc: Option<u32>) -> Result<DocState, Error> {
        let id = doc
            .or(self.front)
            .ok_or_else(|| Error::Refused("No schedule is open.".to_string()))?;
        self.docs
            .iter()
            .find(|(d, _)| *d == id)
            .map(|(_, s)| s.clone())
            .ok_or_else(|| Error::Refused("That schedule is no longer open.".to_string()))
    }

    /// Runs `command` with `args` (the desktop's JSON) and returns its reply as JSON.
    pub fn dispatch(&mut self, command: &str, args_json: &Value) -> Result<Value, Error> {
        match command {
            "host_info" => return reply(Ok(self.host_info())),
            "open_mtsc" => {
                let a: OpenArgs = args(command, args_json)?;
                return reply(Ok(self.open_mtsc(&a.text, a.name.as_deref())?));
            }
            "save_mtsc" => {
                let a: DocArg = args(command, args_json)?;
                return reply(Ok(self.save_mtsc(a.doc)?));
            }
            "close_doc" => {
                let a: DocArg = args(command, args_json)?;
                let id = a.doc.or(self.front).unwrap_or(0);
                self.docs.retain(|(d, _)| *d != id);
                self.saved.remove(&id);
                self.front = self.docs.last().map(|(d, _)| *d);
                return reply(Ok(()));
            }
            "list_docs" => {
                let names: Vec<(u32, String)> = self
                    .docs
                    .iter()
                    .map(|(d, s)| (*d, s.lock().display_name()))
                    .collect();
                return reply(Ok(names));
            }
            _ => {}
        }
        if files::COMMANDS.contains(&command) {
            if self.read_only && !files::READS.contains(&command) {
                return Err(Error::Refused(READ_ONLY.to_string()));
            }
            let a: DocArg = args(command, args_json)?;
            let state = match a.doc {
                Some(_) => Some(self.doc(a.doc)?),
                None => self.doc(None).ok(),
            };
            if let Some(r) = self.run_file(command, args_json, state) {
                return r;
            }
        }
        if !generated::COMMANDS.contains(&command) {
            return Err(Error::UnknownCommand(command.to_string()));
        }
        if self.read_only && !generated::READS.contains(&command) {
            return Err(Error::Refused(READ_ONLY.to_string()));
        }
        let a: DocArg = args(command, args_json)?;
        let state = self.doc(a.doc)?;
        generated::dispatch(command, args_json, &state)
            .unwrap_or_else(|| Err(Error::UnknownCommand(command.to_string())))
    }
}

// ---- What the desktop's command bodies call, with the desktop's signatures, so the
// generated arms read exactly like the commands they mirror. `W` stands in for the window.

/// A desktop command's reply: its value or the message the page shows.
pub(crate) type CmdResult<T = View> = Result<T, String>;

/// The Breakdown tab's stepping order from the page's key, as the desktop reads it.
pub(crate) fn sheet_order(key: Option<&str>) -> CmdResult<method_schedule_core::model::SheetOrder> {
    match key {
        None | Some("") => Ok(method_schedule_core::model::SheetOrder::Board),
        Some(k) => method_schedule_core::model::SheetOrder::parse(k)
            .ok_or_else(|| "Step by board order, sheet number or scene number.".into()),
    }
}

/// The window a desktop command body names; the host has none.
pub(crate) struct W;

/// The view after a command, as the desktop's `finish` gives it (no window to retitle, no
/// autosave switch).
pub(crate) fn finish(_: &W, s: &Session, notice: Option<String>) -> Result<View, String> {
    let mut v = view::view(s, notice)?;
    v.autosave = false;
    Ok(v)
}

/// One edit as one undo step, then the view.
pub(crate) fn edit<T>(
    w: &W,
    state: &DocState,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<T>,
) -> Result<View, String> {
    let mut s = state.lock();
    s.edit(f)?;
    finish(w, &s, None)
}

/// An edit that makes a strip, which the view names.
pub(crate) fn strip_edit(
    w: &W,
    state: &DocState,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<u32>,
) -> Result<View, String> {
    let mut s = state.lock();
    let id = s.edit(f)?;
    let mut v = finish(w, &s, None)?;
    v.strip = Some(id);
    Ok(v)
}

/// An edit that makes a board, which is then shown.
pub(crate) fn board_edit(
    w: &W,
    state: &DocState,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<u32>,
) -> Result<View, String> {
    let mut s = state.lock();
    let id = s.edit(f)?;
    s.show_board(method_schedule_core::model::BoardId(id))?;
    finish(w, &s, None)
}

/// An edit that picks the Breakdown tab's sheet.
pub(crate) fn sheet_edit(
    w: &W,
    state: &DocState,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult<Option<u32>>,
) -> Result<View, String> {
    let mut s = state.lock();
    let show = s.edit(f)?;
    let mut v = finish(w, &s, None)?;
    v.sheet = show;
    Ok(v)
}

/// The layouts module's name for [`edit`].
pub(crate) fn edit_with(
    w: &W,
    state: &DocState,
    f: impl FnOnce(&mut method_schedule_core::Schedule) -> crate::board::EditResult,
) -> Result<View, String> {
    edit(w, state, f)
}

/// Today, on the host's clock (src/clock.rs), where the desktop reads the system's.
pub(crate) fn today() -> Option<method_schedule_core::Date> {
    let ms = crate::clock::wall_ms();
    (ms > 0).then_some(method_schedule_core::Date((ms / 86_400_000) as i32))
}

pub(crate) use reply as to_reply;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn field_station() -> String {
        method_schedule_format::to_string(&method_schedule_core::sample::field_station())
    }

    #[test]
    fn a_schedule_opens_from_its_text_and_saves_back_the_same() {
        let mut h = Host::new();
        let text = field_station();
        let opened = h
            .dispatch(
                "open_mtsc",
                &json!({ "text": text, "name": "field-station.mtsc" }),
            )
            .unwrap();
        assert_eq!(opened["doc"], 1);
        assert_eq!(opened["view"]["title"], "Field Station");
        let saved = h.dispatch("save_mtsc", &json!({})).unwrap();
        assert_eq!(saved["text"], text, "nothing changed, byte for byte");
        assert_eq!(saved["name"], "Field Station.mtsc");
        assert!(h.dispatch("open_mtsc", &json!({ "text": "{}" })).is_err());
    }

    #[test]
    fn the_desktops_commands_run_with_the_desktops_arguments() {
        let mut h = Host::new();
        h.open_mtsc(&field_station(), None).unwrap();
        let v = h.dispatch("get_view", &Value::Null).unwrap();
        let board = v["board"]["id"].as_u64().unwrap();
        let sub = v["board"]["sub_boards"][0]["id"].as_u64().unwrap();
        // A view with snake_case arguments, as the page sends this one.
        let grid = h
            .dispatch("dood_grid", &json!({ "sub_board": sub }))
            .unwrap();
        assert!(grid.is_object());
        // An edit as one undo step: the first scene to the Boneyard, then undone.
        let first = v["board"]["sub_boards"][0]["days"][0]["strips"][0]["id"]
            .as_u64()
            .unwrap();
        let moved = h
            .dispatch(
                "move_strips",
                &json!({ "board": board, "strips": [first], "index": 0 }),
            )
            .unwrap();
        assert_eq!(moved["board"]["boneyard"][0]["id"], first);
        let back = h.dispatch("undo", &Value::Null).unwrap();
        assert_eq!(
            back["board"]["sub_boards"][0]["days"][0]["strips"][0]["id"],
            first
        );
        // Wrong arguments and unknown commands say so.
        assert!(matches!(
            h.dispatch("move_strips", &json!({ "board": "x" })),
            Err(Error::BadArguments { .. })
        ));
        assert!(matches!(
            h.dispatch("no_such_command", &Value::Null),
            Err(Error::UnknownCommand(_))
        ));
    }

    #[test]
    fn read_only_refuses_edits_and_keeps_views() {
        let mut h = Host::new();
        h.open_mtsc(&field_station(), None).unwrap();
        h.set_read_only(true);
        assert!(h.host_info().read_only);
        assert!(h.dispatch("get_view", &Value::Null).is_ok());
        let v = h.dispatch("get_view", &Value::Null).unwrap();
        let board = v["board"]["id"].as_u64().unwrap();
        let e = h
            .dispatch(
                "move_strips",
                &json!({ "board": board, "strips": [1], "index": 0 }),
            )
            .unwrap_err();
        assert_eq!(e, Error::Refused(READ_ONLY.to_string()));
        assert!(h.dispatch("undo", &Value::Null).is_err());
    }

    #[test]
    fn the_command_list_is_the_contract() {
        let all = commands();
        for c in [
            "get_view",
            "move_strips",
            "undo",
            "find_text",
            "report_data",
            "open_mtsc",
            "host_info",
        ] {
            assert!(all.contains(&c), "{c}");
        }
        for r in generated::READS {
            assert!(generated::COMMANDS.contains(r), "{r}");
        }
        assert_eq!(COMMANDS_VERSION, 1);
        assert_eq!(Host::new().host_info().kind, "web");
    }
}
