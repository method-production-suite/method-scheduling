//! Script scenes from Storyteller (decision 229, EXCHANGE.md "Script scenes from
//! Storyteller"): a `.mscenes` file, `method-script-scenes`, read into the same imported
//! script a Fountain or `.fdx` file gives, with Storyteller's own pages and lengths.
//!
//! Unlike our `.mtsc` reader this one ignores keys it doesn't know and reads every version
//! number (the agreed rule: the writer only adds fields after version 1).

use serde::Deserialize;

use method_schedule_core::fountain::{scene_from_heading, ImportedScript, IntExt};
use method_schedule_core::Eighths;

/// The file's `format`.
pub const FORMAT: &str = "method-script-scenes";
/// Its extension.
pub const EXTENSION: &str = "mscenes";
/// The most scenes a file may hold, against a file built to fill memory.
const MAX_SCENES: usize = 20_000;

#[derive(Deserialize)]
struct File {
    format: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    episode: Option<String>,
    #[serde(default)]
    scenes: Vec<Scene>,
}

#[derive(Deserialize)]
struct Scene {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    number: Option<String>,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    int_ext: Option<String>,
    #[serde(default)]
    set: Option<String>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    story_day: Option<String>,
    #[serde(default)]
    page: String,
    eighths: u32,
    #[serde(default)]
    cast: Vec<String>,
    #[serde(default)]
    nonspeaking: Vec<String>,
    #[serde(default)]
    omitted: bool,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    act: Option<String>,
}

fn clean(s: Option<String>) -> String {
    s.map(|x| x.trim().to_string()).unwrap_or_default()
}

/// Reads a script scenes file. Refused, in words, when it isn't one, or when a scene is
/// longer than a sheet may be.
pub fn parse(text: &str) -> Result<ImportedScript, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let probe: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "This isn't a script scenes file.".to_string())?;
    if probe.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err("This isn't a script scenes file.".to_string());
    }
    let f: File = serde_json::from_value(probe)
        .map_err(|e| format!("This script scenes file can't be read: {e}."))?;
    debug_assert_eq!(f.format, FORMAT);
    if f.scenes.len() > MAX_SCENES {
        return Err(format!(
            "This script scenes file has more than {MAX_SCENES} scenes."
        ));
    }
    let episode = clean(f.episode);
    let mut out = ImportedScript {
        title: Some(f.title.trim().to_string()).filter(|t| !t.is_empty()),
        ..ImportedScript::default()
    };
    for (i, s) in f.scenes.into_iter().enumerate() {
        if s.eighths > method_schedule_core::model::MAX_SHEET_EIGHTHS {
            return Err(format!(
                "Scene {} is longer than a sheet may be (999 pages).",
                s.number.as_deref().unwrap_or(&s.heading)
            ));
        }
        // The heading split as the Fountain import splits it, then what the file sent
        // in its place (decision 229: taken as sent when present).
        let mut scene = scene_from_heading(s.heading.trim());
        if let Some(ie) = s.int_ext.as_deref().map(str::trim) {
            scene.int_ext = match ie.to_ascii_uppercase().as_str() {
                "INT" => IntExt::Int,
                "EXT" => IntExt::Ext,
                "INT/EXT" => IntExt::IntExt,
                "EXT/INT" => IntExt::ExtInt,
                _ => scene.int_ext,
            };
        }
        if let Some(set) = s.set {
            scene.set = set.trim().to_string();
        }
        if let Some(time) = s.time {
            scene.time_of_day = time.trim().to_string();
        }
        let place = if s.index > 0 { s.index } else { i as u32 + 1 };
        let number = clean(s.number);
        scene.extra.numbered_by_place = number.is_empty();
        scene.number = Some(if number.is_empty() {
            place.to_string()
        } else {
            number
        });
        scene.length = Eighths(s.eighths.max(1));
        scene.characters = if s.omitted { Vec::new() } else { s.cast };
        scene.extra.nonspeaking = if s.omitted { Vec::new() } else { s.nonspeaking };
        scene.extra.omitted = s.omitted;
        scene.extra.script_page = s.page.trim().to_string();
        scene.extra.story_day = clean(s.story_day);
        scene.extra.sequence = clean(s.act);
        scene.extra.episode = episode.clone();
        scene.extra.synopsis = match (clean(s.title), clean(s.summary)) {
            (t, s) if !t.is_empty() && !s.is_empty() => format!("{t}: {s}"),
            (t, s) if s.is_empty() => t,
            (_, s) => s,
        };
        scene.page = scene.extra.script_page.parse().unwrap_or(0);
        out.scenes.push(scene);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = include_str!("../../../fixtures/scripts/field-station-sample.mscenes");

    #[test]
    fn reads_storytellers_pages_and_the_rest() {
        let s = parse(FILE).unwrap();
        assert_eq!(s.title.as_deref(), Some("Field Station"));
        assert_eq!(s.scenes.len(), 4);
        let a = &s.scenes[0];
        assert_eq!(a.number.as_deref(), Some("1"));
        assert_eq!(a.int_ext, IntExt::Int);
        assert_eq!(a.set, "FIELD STATION - RADIO ROOM");
        assert_eq!(a.time_of_day, "NIGHT");
        assert_eq!(a.length, Eighths(6));
        assert_eq!(a.characters, vec!["MARA".to_string()]);
        assert_eq!(a.extra.nonspeaking, vec!["TEO".to_string()]);
        assert_eq!(a.extra.story_day, "DAY 1");
        assert_eq!(a.extra.synopsis, "The call: Mara tries the ridge base.");
        assert_eq!(a.extra.sequence, "ACT ONE");
        assert_eq!(a.extra.episode, "101");
        // A lettered scene on a locked page, its split taken from the heading.
        let b = &s.scenes[1];
        assert_eq!(b.number.as_deref(), Some("1A"));
        assert_eq!(b.extra.script_page, "1A");
        assert_eq!(
            (b.int_ext, b.set.as_str(), b.time_of_day.as_str()),
            (IntExt::Ext, "RIDGE ROAD", "NIGHT")
        );
        // Omitted: no cast, a length of one eighth.
        let c = &s.scenes[2];
        assert!(c.extra.omitted && c.characters.is_empty());
        assert_eq!(c.length, Eighths(1));
        // No number: its place in the script, and the import says so.
        let d = &s.scenes[3];
        assert_eq!(d.number.as_deref(), Some("4"));
        assert!(d.extra.numbered_by_place);
        assert!(!a.extra.numbered_by_place);
    }

    #[test]
    fn ignores_keys_it_does_not_know_and_refuses_other_files() {
        let mut v: serde_json::Value = serde_json::from_str(FILE).unwrap();
        v["version"] = serde_json::json!(7);
        v["something_new"] = serde_json::json!({"a": 1});
        v["scenes"][0]["also_new"] = serde_json::json!(true);
        assert_eq!(parse(&v.to_string()).unwrap().scenes.len(), 4);
        assert!(parse("{}").is_err());
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"format":"method-schedule-days","version":1}"#).is_err());
        let mut long = v.clone();
        long["scenes"][0]["eighths"] = serde_json::json!(8000);
        assert!(parse(&long.to_string()).unwrap_err().contains("longer"));
    }

    #[test]
    fn imports_into_a_schedule_with_every_field() {
        let script = parse(FILE).unwrap();
        let mut s = method_schedule_core::Schedule::with_default_categories("Field Station");
        let cast = s.cast_or_add();
        let sheets = s.import_script(&script, cast).unwrap();
        let h = s.sheet(sheets[0]).unwrap();
        assert_eq!(h.script_day, "DAY 1");
        assert_eq!(h.sequence, "ACT ONE");
        assert_eq!(h.episode, "101");
        assert_eq!(h.script_pages, "1");
        assert_eq!(h.elements.len(), 2, "MARA speaking, TEO not");
        assert!(s.plan.is_omitted(sheets[2]));
        assert!(!s.plan.is_omitted(sheets[0]));
        s.check().unwrap();
    }
}
