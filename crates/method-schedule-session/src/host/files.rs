//! The desktop commands that show an Open or Save dialog, for a host with no file system:
//! a file the person picked comes in as [`FileIn`] (its name and its bytes as base64) where
//! the desktop shows an Open dialog, and a file to keep goes back in [`Downloads`] where
//! the desktop shows a Save dialog and writes it. Pictures (an element's or a sheet's) are
//! kept by the host in its picture store ([`Host::put_image`]), by the name the schedule
//! stores. Method Budget's host has the same shapes.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use method_schedule_core::fountain;

use super::{args, finish, reply, DocState, Error, Host, W};
use crate::changes::compare;
use crate::export::{budget_text, file_bytes, workbook, ExportOutcome, Format, Report};
use crate::layouts::{layouts_file, load_layouts, LAYOUTS_EXTENSION};
use crate::plan::{call_sheet_bytes, revision_preview, upper, RevisionDto};
use crate::session::{self, Session};
use crate::{elements, split, templates};
use method_schedule_core::revise::{revise, revision_text};

/// The commands here, each taking a file (`file`) or giving files back (`downloads`).
pub(super) const COMMANDS: &[&str] = &[
    "new_schedule",
    "open_schedule",
    "open_sample",
    "new_from_template",
    "save_schedule",
    "save_template",
    "import_script",
    "import_script_tags",
    "import_layouts",
    "export_layouts",
    "export_report",
    "export_budget",
    "export_call_sheet",
    "revise_pick",
    "revise_apply",
    "what_changed",
    "add_element_image",
    "add_sheet_image",
    "element_image",
    "sheet_image",
];

/// The ones that change no open schedule: allowed while the host is read-only. Opening a
/// file opens a new schedule beside the others, which is read-only too.
pub(super) const READS: &[&str] = &[
    "new_schedule",
    "open_schedule",
    "open_sample",
    "new_from_template",
    "save_schedule",
    "save_template",
    "export_layouts",
    "export_report",
    "export_budget",
    "export_call_sheet",
    "element_image",
    "sheet_image",
    "revise_pick",
    "what_changed",
];

/// A file the person picked, as a host hands it in where the desktop shows an Open
/// dialog. The name is only shown and read for its extension (no path); the bytes are one
/// base64 string in the JSON ([`super::b64_encode`]).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FileIn {
    pub name: String,
    #[serde(with = "super::b64")]
    pub bytes: Vec<u8>,
}

/// A file for the person to keep, where the desktop shows a Save dialog and writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FileOut {
    pub name: String,
    #[serde(with = "super::b64")]
    pub bytes: Vec<u8>,
}

/// The reply of a command that saves a file on the desktop: the desktop's reply as after
/// the save (a file's name where the desktop names the file it wrote), and beside it
/// `downloads`, the files themselves, for the host to offer.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Downloads<T> {
    #[serde(flatten)]
    pub reply: T,
    pub downloads: Vec<FileOut>,
}

impl Downloads<ExportOutcome> {
    /// One file, reported as the desktop reports a finished export.
    pub fn one(name: String, bytes: Vec<u8>) -> Downloads<ExportOutcome> {
        Downloads {
            reply: ExportOutcome {
                done: true,
                file: Some(name.clone()),
            },
            downloads: vec![FileOut { name, bytes }],
        }
    }
}

/// A revision picked and previewed by `revise_pick`, applied by `revise_apply` with its
/// token: the script is read once, so the merge is the one the preview showed.
pub(super) struct Revision {
    token: u64,
    doc: Option<u32>,
    script: fountain::ImportedScript,
    tags: bool,
}

/// A file in the most a schedule file may be (the desktop's `read_limited`).
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct FileArg {
    file: FileIn,
    #[serde(default)]
    lines_per_page: Option<u32>,
}

#[derive(Deserialize)]
struct ReviseArgs {
    file: FileIn,
    #[serde(default)]
    tags: bool,
    #[serde(default)]
    lines_per_page: Option<u32>,
}

#[derive(Deserialize)]
struct TokenArg {
    token: u64,
    #[serde(default)]
    doc: Option<u32>,
}

#[derive(Deserialize)]
struct AgainstArgs {
    against: String,
    #[serde(default)]
    file: Option<FileIn>,
    #[serde(default)]
    doc: Option<u32>,
}

#[derive(Deserialize)]
struct SubArg {
    sub_board: u32,
}

#[derive(Deserialize)]
struct ReportArgs {
    sub_board: u32,
    report: String,
    format: String,
}

#[derive(Deserialize)]
struct CallSheetArgs {
    sub_board: u32,
    day: u32,
    format: String,
}

#[derive(Deserialize)]
struct IdsArg {
    #[serde(default)]
    ids: Vec<u32>,
}

#[derive(Deserialize)]
struct ImageArgs {
    #[serde(alias = "sheet")]
    element: u32,
    file: FileIn,
}

#[derive(Deserialize)]
struct ImageAtArgs {
    #[serde(alias = "sheet")]
    element: u32,
    at: usize,
}

fn file_arg(command: &str, a: &Value) -> Result<FileArg, Error> {
    let f: FileArg = args(command, a)?;
    if f.file.bytes.len() > MAX_FILE_BYTES {
        return Err(Error::Refused(format!(
            "{} is too large to open.",
            f.file.name
        )));
    }
    Ok(f)
}

fn text(file: &FileIn, what: &str) -> Result<String, Error> {
    let t = String::from_utf8(file.bytes.clone())
        .map_err(|_| Error::Refused(format!("That isn't {what} Method Schedule can read.")))?;
    Ok(t.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(t))
}

/// The schedule's file name stem, as the desktop's exports start their names.
fn stem(s: &Session) -> String {
    s.path()
        .and_then(Path::file_stem)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| crate::export::safe_file_stem(&s.schedule().title))
}

/// A picture's name in the picture store: `images/` and the picked file's name, made safe
/// and kept apart from a different picture of the same name.
fn image_name(host: &Host, file: &FileIn) -> String {
    let base = Path::new(&file.name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = crate::export::safe_file_stem(&base);
    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (base.clone(), String::new()),
    };
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("images/{stem}{ext}")
        } else {
            format!("images/{stem} ({n}){ext}")
        };
        match host.images.get(&name) {
            Some(b) if *b != file.bytes => n += 1,
            _ => return name,
        }
    }
}

impl Host {
    /// Keeps a picture's bytes under the name a schedule stores for it (`images/...`), as
    /// a host restores its picture store when it opens a schedule it kept.
    pub fn put_image(&mut self, name: &str, bytes: Vec<u8>) {
        self.images.insert(name.to_string(), bytes);
    }

    /// The bytes kept under a picture's name.
    pub fn image(&self, name: &str) -> Option<&[u8]> {
        self.images.get(name).map(Vec::as_slice)
    }

    /// Every picture's name, for a host saving its picture store beside a schedule.
    pub fn image_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.images.keys().cloned().collect();
        v.sort();
        v
    }

    /// A picked picture into the store, or why it can't be one.
    fn take_image(&mut self, file: &FileIn) -> Result<String, Error> {
        if elements::image_type(Path::new(&file.name)).is_none()
            || elements::image_data_url(&file.bytes).is_none()
        {
            return Err(Error::Refused(format!(
                "{} isn't a picture Method Schedule can show (PNG, JPEG, GIF or WebP, up to 8 MB).",
                file.name
            )));
        }
        let name = image_name(self, file);
        self.images.insert(name.clone(), file.bytes.clone());
        Ok(name)
    }

    /// Runs one of [`COMMANDS`] on `state` (the schedule in front, or `doc`), or `None`
    /// for any other command.
    pub(super) fn run_file(
        &mut self,
        command: &str,
        a: &Value,
        state: Option<DocState>,
    ) -> Option<Result<Value, Error>> {
        if !COMMANDS.contains(&command) {
            return None;
        }
        let open = |host: &mut Host, s: Session, notice: Option<String>| -> Result<Value, Error> {
            reply(Ok(host.add_doc(s, notice)?))
        };
        let need = || {
            state
                .clone()
                .ok_or_else(|| Error::Refused("No schedule is open.".into()))
        };
        Some((|| -> Result<Value, Error> {
            match command {
                "new_schedule" => open(self, Session::new(), None),
                "open_sample" => open(
                    self,
                    session::sample_session()?,
                    Some(session::SAMPLE_NOTICE.into()),
                ),
                "open_schedule" => {
                    let f = file_arg(command, a)?;
                    let schedule = session::parse_schedule(&f.file.bytes, Path::new(&f.file.name))?;
                    let s = Session::with_schedule(schedule, Some(f.file.name.clone().into()));
                    reply(Ok(self.add_saved_doc(s)?))
                }
                "new_from_template" => {
                    let f = file_arg(command, a)?;
                    let schedule = session::parse_schedule(&f.file.bytes, Path::new(&f.file.name))?;
                    let notice = format!("A new schedule from the template {}.", f.file.name);
                    open(self, templates::session_from(&schedule), Some(notice))
                }
                "save_schedule" => {
                    let state = need()?;
                    let s = state.lock();
                    let name = format!("{}.{}", stem(&s), session::EXTENSION);
                    let view = finish(&W, &s, None)?;
                    let doc = args::<super::DocArg>(command, a)?.doc.or(self.front);
                    if let Some(id) = doc {
                        self.saved.insert(id, s.schedule().clone());
                    }
                    reply(Ok(Downloads {
                        reply: view,
                        downloads: vec![FileOut {
                            name,
                            bytes: s.file_text().into_bytes(),
                        }],
                    }))
                }
                "save_template" => {
                    let state = need()?;
                    let s = state.lock();
                    let name = format!(
                        "{}.{}",
                        crate::export::safe_file_stem(&s.schedule().title),
                        templates::EXTENSION
                    );
                    let bytes = templates::template_text(s.schedule()).into_bytes();
                    reply(Ok(Downloads::one(name, bytes)))
                }
                "import_script" | "import_script_tags" => {
                    let f = file_arg(command, a)?;
                    let state = need()?;
                    let lines = f.lines_per_page.unwrap_or(fountain::LINES_PER_PAGE);
                    let mut s = state.lock();
                    let tags = command == "import_script_tags";
                    let script = script_of(&f.file, tags, lines)?;
                    let notice = if tags {
                        tagged(&mut s, &script)?
                    } else {
                        session::import_notice(s.import_script(&script)?)
                    };
                    reply(finish(&W, &s, Some(notice)))
                }
                "import_layouts" => {
                    let f = file_arg(command, a)?;
                    let text = text(&f.file, "a layout file")?;
                    let state = need()?;
                    let mut s = state.lock();
                    let n = s.edit(|x| load_layouts(x, &text))?;
                    let what = if n == 1 {
                        "1 layout".to_string()
                    } else {
                        format!("{n} layouts")
                    };
                    reply(finish(&W, &s, Some(format!("Loaded {what}."))))
                }
                "export_layouts" => {
                    let i: IdsArg = args(command, a)?;
                    let state = need()?;
                    let s = state.lock();
                    let (text, _) = layouts_file(s.schedule(), &i.ids)?;
                    let name = format!(
                        "{} - Layouts.{LAYOUTS_EXTENSION}",
                        crate::export::safe_file_stem(&s.schedule().title)
                    );
                    reply(Ok(Downloads::one(name, text.into_bytes())))
                }
                "export_report" => {
                    let r: ReportArgs = args(command, a)?;
                    let report =
                        Report::parse(&r.report).ok_or("That report can't be exported.")?;
                    let format = Format::parse(&r.format)
                        .ok_or("That export format isn't one the app knows.")?;
                    let state = need()?;
                    let s = state.lock();
                    let bytes = file_bytes(&workbook(s.schedule(), r.sub_board, report)?, format)?;
                    let name = format!(
                        "{} - {}.{}",
                        stem(&s),
                        report.file_label(),
                        format.extension()
                    );
                    reply(Ok(Downloads::one(name, bytes)))
                }
                "export_budget" => {
                    let r: SubArg = args(command, a)?;
                    let state = need()?;
                    let s = state.lock();
                    let text = budget_text(s.schedule(), r.sub_board, super::today())?;
                    let name = format!("{} - Budget Days.json", stem(&s));
                    reply(Ok(Downloads::one(name, text.into_bytes())))
                }
                "export_call_sheet" => {
                    let r: CallSheetArgs = args(command, a)?;
                    let csv = match r.format.as_str() {
                        "csv" => true,
                        "json" => false,
                        _ => {
                            return Err("That call sheet format isn't one the app knows."
                                .to_string()
                                .into())
                        }
                    };
                    let state = need()?;
                    let s = state.lock();
                    let bytes = call_sheet_bytes(s.schedule(), r.sub_board, r.day, csv)?;
                    let ext = if csv { "csv" } else { "json" };
                    let name = format!("{} - Call Sheet Day {}.{ext}", stem(&s), r.day);
                    reply(Ok(Downloads::one(name, bytes)))
                }
                "revise_pick" => {
                    let r: ReviseArgs = args(command, a)?;
                    if r.file.bytes.len() > MAX_FILE_BYTES {
                        return Err(format!("{} is too large to open.", r.file.name).into());
                    }
                    let state = need()?;
                    let lines = r.lines_per_page.unwrap_or(fountain::LINES_PER_PAGE);
                    let script = script_of(&r.file, r.tags, lines)?;
                    let (matched, changed, added, omitted, lines) =
                        revision_preview(state.lock().schedule(), &script, r.tags)?;
                    self.tokens += 1;
                    let token = self.tokens;
                    let doc = args::<super::DocArg>(command, a)?.doc.or(self.front);
                    self.revision = Some(Revision {
                        token,
                        doc,
                        script,
                        tags: r.tags,
                    });
                    reply(Ok(RevisionDto {
                        token,
                        file: r.file.name,
                        matched,
                        changed,
                        added,
                        omitted,
                        lines,
                    }))
                }
                "revise_apply" => {
                    let t: TokenArg = args(command, a)?;
                    let doc = t.doc.or(self.front);
                    let pending = self
                        .revision
                        .take()
                        .filter(|x| x.token == t.token && x.doc == doc)
                        .ok_or("That revision is no longer waiting; pick the script again.")?;
                    let state = need()?;
                    let mut s = state.lock();
                    let first = s.edit(|x| {
                        let cast = x.cast_or_add();
                        let r = revise(x, &pending.script, cast, pending.tags).map_err(upper)?;
                        Ok(revision_text(x, &r)
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_string())
                    })?;
                    let notice = format!("Revision merged: {first} Undo takes it back.");
                    reply(finish(&W, &s, Some(notice)))
                }
                "what_changed" => {
                    let w: AgainstArgs = args(command, a)?;
                    let state = need()?;
                    let name = state.lock().display_name();
                    let (older, from) = match w.against.as_str() {
                        "file" => {
                            let f = w.file.ok_or("Pick a schedule to compare with.")?;
                            let older = session::parse_schedule(&f.bytes, Path::new(&f.name))?;
                            (older, f.name)
                        }
                        "saved" => {
                            let id = w.doc.or(self.front).unwrap_or(0);
                            let older = self
                                .saved
                                .get(&id)
                                .cloned()
                                .ok_or("This schedule hasn't been saved to a file yet.")?;
                            (older, format!("{name} as last saved"))
                        }
                        _ => return Err("Unknown comparison.".into()),
                    };
                    let newer = state.lock().schedule().clone();
                    reply(Ok(compare(
                        &older,
                        &newer,
                        from,
                        format!("{name} as it is now"),
                    )))
                }
                "add_element_image" | "add_sheet_image" => {
                    let i: ImageArgs = args(command, a)?;
                    let state = need()?;
                    let stored = self.take_image(&i.file)?;
                    let mut s = state.lock();
                    if command == "add_element_image" {
                        s.edit(|x| elements::add_image(x, i.element, stored))?;
                    } else {
                        s.edit(|x| split::add_image(x, i.element, stored))?;
                    }
                    reply(finish(&W, &s, None))
                }
                "element_image" | "sheet_image" => {
                    let i: ImageAtArgs = args(command, a)?;
                    let state = need()?;
                    let s = state.lock();
                    let stored = if command == "element_image" {
                        elements::image_of(s.schedule(), i.element, i.at)?
                    } else {
                        split::image_of(s.schedule(), i.element, i.at)?
                    };
                    let url = self.image(&stored).and_then(elements::image_data_url);
                    reply(Ok(url))
                }
                _ => Err(Error::UnknownCommand(command.to_string())),
            }
        })())
    }
}

/// A picked script, with tags (`.fdx` only) or without; a Fountain one at `lines` a page.
fn script_of(file: &FileIn, tags: bool, lines: u32) -> Result<fountain::ImportedScript, Error> {
    if !tags {
        return Ok(session::parse_script(
            file.bytes.clone(),
            Path::new(&file.name),
            lines,
        )?);
    }
    let fdx_file = Path::new(&file.name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fdx"));
    if !fdx_file {
        return Err("Production tags come only from Final Draft (.fdx) files.".into());
    }
    let text = text(file, "a script")?;
    Ok(method_schedule_core::fdx::parse_with_tags(&text).map_err(upper)?)
}

/// Imports a script read with its production tags (decisions.md, IMP-04), as the
/// desktop's Import Script with Tags; the notice it shows.
fn tagged(s: &mut Session, script: &fountain::ImportedScript) -> Result<String, Error> {
    let tags: usize = script.scenes.iter().map(|s| s.tags.len()).sum();
    let added = s.edit(|x| {
        let cast = x.cast_or_add();
        method_schedule_core::revise::import_tagged(x, script, cast)
            .map(|v| v.len())
            .map_err(upper)
    })?;
    Ok(format!(
        "Added {added} {} to the Boneyard, with {tags} production {}.",
        if added == 1 { "scene" } else { "scenes" },
        if tags == 1 { "tag" } else { "tags" }
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{b64_encode, Host};
    use serde_json::{json, Value};

    fn file(name: &str, bytes: &[u8]) -> Value {
        json!({ "name": name, "bytes": b64_encode(bytes) })
    }

    /// A 1 by 1 PNG, the smallest picture the sniffer takes.
    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
        0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0,
    ];

    fn sample() -> (Host, Value) {
        let mut h = Host::new();
        let v = h.dispatch("open_sample", &Value::Null).unwrap();
        (h, v)
    }

    #[test]
    fn the_file_commands_are_their_own() {
        for c in super::COMMANDS {
            assert!(!super::super::COMMANDS.contains(c), "{c} is generated too");
        }
        for c in super::READS {
            assert!(super::COMMANDS.contains(c), "{c}");
        }
        assert!(super::super::commands().contains(&"export_report"));
    }

    #[test]
    fn schedules_open_from_files_and_save_as_downloads() {
        let (mut h, opened) = sample();
        assert_eq!(opened["doc"], 1);
        assert!(opened["view"]["notice"]
            .as_str()
            .unwrap()
            .contains("Field Station"));
        let saved = h.dispatch("save_schedule", &json!({})).unwrap();
        let d = &saved["downloads"][0];
        assert!(d["name"].as_str().unwrap().ends_with(".mtsc"), "{d}");
        let bytes = super::super::b64_decode(d["bytes"].as_str().unwrap()).unwrap();
        assert!(
            saved["title"].is_string(),
            "the reply is the view, as the desktop's"
        );
        // The download opens again as a second schedule, the same schedule.
        let again = h
            .dispatch(
                "open_schedule",
                &json!({ "file": file("again.mtsc", &bytes) }),
            )
            .unwrap();
        assert_eq!(again["doc"], 2);
        assert_eq!(again["view"]["title"], opened["view"]["title"]);
        let t = h.dispatch("save_template", &json!({ "doc": 2 })).unwrap();
        let tb = super::super::b64_decode(t["downloads"][0]["bytes"].as_str().unwrap()).unwrap();
        assert_eq!(t["done"], true);
        let from = h
            .dispatch("new_from_template", &json!({ "file": file("t.mtst", &tb) }))
            .unwrap();
        assert_eq!(from["view"]["title"], "Untitled");
        let bad = h.dispatch("open_schedule", &json!({ "file": file("x.mtsc", b"{}") }));
        assert!(bad.unwrap_err().message().starts_with("x.mtsc"));
        let not_b64 = h.dispatch(
            "open_schedule",
            &json!({ "file": { "name": "x", "bytes": "*" } }),
        );
        assert!(matches!(not_b64, Err(super::Error::BadArguments { .. })));
    }

    #[test]
    fn a_script_imports_from_its_bytes() {
        let mut h = Host::new();
        h.dispatch("new_schedule", &Value::Null).unwrap();
        let script = b"Title: Dry Creek\n\nINT. RANGER STATION - DAY\n\nA radio crackles.\n\nEXT. RIDGE - NIGHT\n\nWind.\n";
        let v = h
            .dispatch(
                "import_script",
                &json!({ "file": file("dry-creek.fountain", script) }),
            )
            .unwrap();
        assert_eq!(v["title"], "Dry Creek");
        assert!(
            v["notice"].as_str().unwrap().contains('2'),
            "{}",
            v["notice"]
        );
        let tags = h.dispatch(
            "import_script_tags",
            &json!({ "file": file("dry-creek.fountain", script) }),
        );
        assert!(tags.unwrap_err().message().contains(".fdx"));
    }

    #[test]
    fn exports_come_back_as_downloads_named_as_the_desktop_names_them() {
        let (mut h, v) = sample();
        let sub = v["view"]["board"]["sub_boards"][0]["id"].as_u64().unwrap();
        let r = h
            .dispatch(
                "export_report",
                &json!({ "sub_board": sub, "report": "dood", "format": "csv" }),
            )
            .unwrap();
        assert_eq!(r["done"], true);
        let name = r["downloads"][0]["name"].as_str().unwrap();
        assert!(name.ends_with(" - Day Out of Days.csv"), "{name}");
        assert_eq!(r["file"], name);
        let x = h
            .dispatch(
                "export_report",
                &json!({ "sub_board": sub, "report": "one_line", "format": "xlsx" }),
            )
            .unwrap();
        let bytes = super::super::b64_decode(x["downloads"][0]["bytes"].as_str().unwrap()).unwrap();
        assert_eq!(&bytes[..2], b"PK", "a workbook is a zip");
        let budget = h.dispatch("export_budget", &json!({ "sub_board": sub }));
        assert!(budget.is_ok() || budget.unwrap_err().message().contains("start date"));
        let cs = h
            .dispatch(
                "export_call_sheet",
                &json!({ "sub_board": sub, "day": 1, "format": "json" }),
            )
            .unwrap();
        assert!(cs["downloads"][0]["name"]
            .as_str()
            .unwrap()
            .ends_with("Call Sheet Day 1.json"));
        // The sample has no named layouts: refused as the desktop refuses it.
        let lay = h.dispatch("export_layouts", &json!({}));
        assert!(lay.unwrap_err().message().contains("no layouts"));
        let bad = h.dispatch(
            "import_layouts",
            &json!({ "file": file("l.mtlayouts", b"[]") }),
        );
        assert!(bad.is_err());
        assert!(h
            .dispatch(
                "export_report",
                &json!({ "sub_board": sub, "report": "x", "format": "csv" })
            )
            .is_err());
    }

    #[test]
    fn pictures_go_in_the_store_and_come_back_as_thumbnails() {
        let (mut h, v) = sample();
        let _ = v;
        let element = method_schedule_core::sample::field_station().elements[0]
            .id
            .0;
        h.dispatch(
            "add_element_image",
            &json!({ "element": element, "file": file("Ranger Truck.png", PNG) }),
        )
        .unwrap();
        assert_eq!(h.image_names(), vec!["images/Ranger Truck.png".to_string()]);
        let url = h
            .dispatch("element_image", &json!({ "element": element, "at": 0 }))
            .unwrap();
        assert!(url.as_str().unwrap().starts_with("data:image/png;base64,"));
        // Another picture of the same name is kept apart; not a picture is refused.
        let mut other = PNG.to_vec();
        other.push(0);
        h.dispatch(
            "add_element_image",
            &json!({ "element": element, "file": file("Ranger Truck.png", &other) }),
        )
        .unwrap();
        assert!(h.image("images/Ranger Truck (2).png").is_some());
        assert!(h
            .dispatch(
                "add_element_image",
                &json!({ "element": element, "file": file("notes.txt", b"hello") }),
            )
            .is_err());
        // Read-only: pictures show, but none is added.
        h.set_read_only(true);
        assert!(h
            .dispatch("element_image", &json!({ "element": element, "at": 0 }))
            .is_ok());
        let refused = h.dispatch(
            "add_element_image",
            &json!({ "element": element, "file": file("b.png", PNG) }),
        );
        assert_eq!(refused.unwrap_err().message(), super::super::READ_ONLY);
    }

    #[test]
    fn a_revision_previews_then_merges_with_its_token() {
        let mut h = Host::new();
        h.dispatch("new_schedule", &Value::Null).unwrap();
        let first =
            b"INT. RANGER STATION - DAY\n\nA radio crackles.\n\nEXT. RIDGE - NIGHT\n\nWind.\n";
        h.dispatch(
            "import_script",
            &json!({ "file": file("v1.fountain", first) }),
        )
        .unwrap();
        let second = b"INT. RANGER STATION - DAY\n\nA radio crackles.\n\nEXT. RIDGE - NIGHT\n\nWind.\n\nINT. TRUCK - NIGHT\n\nHeadlights.\n";
        let p = h
            .dispatch(
                "revise_pick",
                &json!({ "file": file("v2.fountain", second) }),
            )
            .unwrap();
        assert_eq!(p["added"], 1, "{p}");
        assert_eq!(p["file"], "v2.fountain");
        let token = p["token"].as_u64().unwrap();
        assert!(h
            .dispatch("revise_apply", &json!({ "token": token + 1 }))
            .is_err());
        // A wrong token drops the waiting revision, as the desktop's does: pick again.
        let p = h
            .dispatch(
                "revise_pick",
                &json!({ "file": file("v2.fountain", second) }),
            )
            .unwrap();
        let v = h
            .dispatch("revise_apply", &json!({ "token": p["token"] }))
            .unwrap();
        assert!(v["notice"]
            .as_str()
            .unwrap()
            .starts_with("Revision merged:"));
        assert!(h
            .dispatch("revise_apply", &json!({ "token": p["token"] }))
            .is_err());
    }

    #[test]
    fn what_changed_compares_with_a_file_or_the_last_save() {
        let mut h = Host::new();
        let text =
            method_schedule_format::to_string(&method_schedule_core::sample::field_station());
        let board = h.open_mtsc(&text, Some("fs.mtsc")).unwrap().view.board.id;
        let none = h
            .dispatch("what_changed", &json!({ "against": "saved" }))
            .unwrap();
        assert_eq!(none["count"], 0);
        h.dispatch(
            "rename_board",
            &json!({ "board": board, "name": "Second unit" }),
        )
        .unwrap();
        let one = h
            .dispatch("what_changed", &json!({ "against": "saved" }))
            .unwrap();
        assert!(one["count"].as_u64().unwrap() >= 1, "{one}");
        assert_eq!(one["from"], "fs.mtsc as last saved");
        let vs = h
            .dispatch(
                "what_changed",
                &json!({ "against": "file", "file": file("old.mtsc", text.as_bytes()) }),
            )
            .unwrap();
        assert_eq!(vs["count"], one["count"]);
        h.dispatch("save_mtsc", &json!({})).unwrap();
        let after = h
            .dispatch("what_changed", &json!({ "against": "saved" }))
            .unwrap();
        assert_eq!(after["count"], 0, "saving moves the mark");
        h.dispatch("new_schedule", &Value::Null).unwrap();
        assert!(h
            .dispatch("what_changed", &json!({ "against": "saved" }))
            .is_err());
    }
}
