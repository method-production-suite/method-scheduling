//! Fountain import: scene headings and speaking characters, with page lengths in eighths.
//!
//! Written from the open spec at fountain.io/syntax. By default an import brings in what
//! the reference product brings in from a plain script: scene headings and the characters
//! who speak (decisions.md, entry 8). Fountain has no page lengths, so we paginate
//! ourselves with the rule in entry 9.

use crate::Eighths;

/// Lines on a script page unless the user picks another count: 56, so an eighth is 7
/// lines (decision 195; it was 55 under decision 9).
pub const LINES_PER_PAGE: u32 = 56;
/// The fewest lines a page may be set to.
pub const MIN_LINES_PER_PAGE: u32 = 40;
/// The most lines a page may be set to.
pub const MAX_LINES_PER_PAGE: u32 = 70;
/// Characters per line for action, at 10 characters an inch on a 6 inch column.
pub const ACTION_WIDTH: usize = 60;
/// Characters per line for dialogue.
pub const DIALOGUE_WIDTH: usize = 35;
/// Characters per line for a parenthetical.
pub const PAREN_WIDTH: usize = 25;

/// Interior or exterior, from the start of a scene heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntExt {
    Int,
    Ext,
    IntExt,
    ExtInt,
    /// A forced heading with no INT or EXT prefix.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedScene {
    /// The scene number from `#...#`, if the script has one.
    pub number: Option<String>,
    /// The heading as written, without the scene number or a forcing period.
    pub heading: String,
    pub int_ext: IntExt,
    /// The set: the heading between the INT/EXT prefix and the time of day.
    pub set: String,
    /// The text after the last ` - `, if any (`DAY`, `NIGHT`, `CONTINUOUS`).
    pub time_of_day: String,
    /// Speaking characters in order of first line, without extensions like `(V.O.)`.
    pub characters: Vec<String>,
    /// Page the heading falls on, from 1.
    pub page: u32,
    pub length: Eighths,
    /// Production tags read from the script (`.fdx` only, when asked for), in order of
    /// first appearance, each once.
    pub tags: Vec<ImportedTag>,
}

/// A tagged item in a script: an element to be, and the name of its category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedTag {
    pub category: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportedScript {
    pub title: Option<String>,
    pub scenes: Vec<ImportedScene>,
}

/// Parses Fountain text at [`LINES_PER_PAGE`]. Never fails: anything unrecognized is
/// action.
pub fn parse(text: &str) -> ImportedScript {
    parse_with(text, LINES_PER_PAGE)
}

/// Parses Fountain text with `lines_per_page` lines a page, held to
/// [`MIN_LINES_PER_PAGE`]..=[`MAX_LINES_PER_PAGE`] (decision 195).
pub fn parse_with(text: &str, lines_per_page: u32) -> ImportedScript {
    let per_page = lines_per_page.clamp(MIN_LINES_PER_PAGE, MAX_LINES_PER_PAGE);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let text = strip_spans(&strip_spans(&text, "/*", "*/"), "[[", "]]");
    let lines: Vec<&str> = text.split('\n').filter(|l| !l.contains(GONE)).collect();

    let (title, body_start) = title_page(&lines);
    let lines = &lines[body_start..];

    let mut out = ImportedScript {
        title,
        scenes: Vec::new(),
    };
    let mut pos: u32 = 0; // lines used so far
    let mut first_element = true;
    let mut scene_start: u32 = 0;
    let mut i = 0;

    // Adds an element of `height` lines, with one blank line before it.
    let place = |pos: &mut u32, first: &mut bool, height: u32| {
        if !*first {
            *pos = pos.saturating_add(1);
        }
        *first = false;
        *pos = pos.saturating_add(height);
    };

    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        let prev_blank = i == 0 || lines[i - 1].trim().is_empty();
        let next_blank = i + 1 >= lines.len() || lines[i + 1].trim().is_empty();

        if t.is_empty() || t.starts_with('#') || is_synopsis(t) {
            i += 1;
            continue;
        }
        if is_page_break(t) {
            if !pos.is_multiple_of(per_page) {
                pos = pos.saturating_add(per_page - pos % per_page);
            }
            first_element = true;
            i += 1;
            continue;
        }
        if prev_blank && is_heading(t, next_blank) {
            if let Some(scene) = out.scenes.last_mut() {
                scene.length = length(scene_start, pos, per_page);
            }
            scene_start = pos;
            place(&mut pos, &mut first_element, 1);
            let mut scene = heading(t);
            scene.page = (pos - 1) / per_page + 1;
            out.scenes.push(scene);
            i += 1;
            continue;
        }
        if prev_blank && !next_blank && is_character(t) {
            let name = character_name(t);
            if let Some(scene) = out.scenes.last_mut() {
                if !name.is_empty() && !scene.characters.contains(&name) {
                    scene.characters.push(name);
                }
            }
            let mut height = 1;
            i += 1;
            // A line of exactly two spaces keeps the speech going: a second paragraph of
            // the same dialogue (Fountain's rule), one blank line on the page.
            while i < lines.len() && (!lines[i].trim().is_empty() || is_dialogue_gap(lines[i])) {
                if is_dialogue_gap(lines[i]) {
                    height += 1;
                    i += 1;
                    continue;
                }
                let d = lines[i].trim();
                let width = if d.starts_with('(') {
                    PAREN_WIDTH
                } else {
                    DIALOGUE_WIDTH
                };
                height += wrapped(d, width);
                i += 1;
            }
            place(&mut pos, &mut first_element, height);
            continue;
        }
        // Action, transitions, centered text, lyrics: a paragraph until the next blank line.
        let mut height = 0;
        while i < lines.len() && !lines[i].trim().is_empty() {
            height += wrapped(lines[i].trim(), ACTION_WIDTH);
            i += 1;
        }
        place(&mut pos, &mut first_element, height);
    }
    if let Some(scene) = out.scenes.last_mut() {
        scene.length = length(scene_start, pos, per_page);
    }
    out
}

/// Marks a line that held only removed text, so it is dropped instead of becoming blank.
const GONE: char = '\u{0}';

/// Removes every `open ... close` span. An unclosed span runs to the end. A line left
/// with nothing but removed text is marked so the caller drops it.
fn strip_spans(text: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        out.push_str(&rest[..start]);
        out.push(GONE);
        let after = &rest[start + open.len()..];
        match after.find(close) {
            Some(end) => {
                // Keep line breaks inside the span so line counts stay right, but mark
                // every line it touches.
                for _ in after[..end].matches('\n') {
                    out.push('\n');
                    out.push(GONE);
                }
                rest = &after[end + close.len()..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    // Unmark lines that still have real text, so only fully removed lines are dropped.
    out.split('\n')
        .map(|l| {
            if l.contains(GONE) && !l.replace(GONE, "").trim().is_empty() {
                l.replace(GONE, "")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A title page is `Key: value` lines at the very top, ending at the first blank line.
/// Returns the title and where the script body starts.
fn title_page(lines: &[&str]) -> (Option<String>, usize) {
    let is_key = |l: &str| {
        l.split_once(':').is_some_and(|(k, _)| {
            !k.is_empty()
                && k.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
                && !k.starts_with(' ')
        })
    };
    if lines.first().is_none_or(|l| !is_key(l)) {
        return (None, 0);
    }
    let end = lines
        .iter()
        .position(|l| l.trim().is_empty())
        .unwrap_or(lines.len());
    let mut title: Option<String> = None;
    let mut in_title = false;
    for l in &lines[..end] {
        if is_key(l) && !l.starts_with([' ', '\t']) {
            let (k, v) = l.split_once(':').unwrap();
            in_title = k.trim().eq_ignore_ascii_case("title");
            if in_title && !v.trim().is_empty() {
                title = Some(plain(v.trim()));
            }
        } else if in_title {
            let v = plain(l.trim());
            title = Some(match title {
                Some(t) => format!("{t} {v}"),
                None => v,
            });
        }
    }
    (title, end)
}

/// Drops emphasis markers.
fn plain(s: &str) -> String {
    s.replace(['*', '_'], "")
}

fn is_page_break(t: &str) -> bool {
    t.len() >= 3 && t.bytes().all(|b| b == b'=')
}

fn is_synopsis(t: &str) -> bool {
    t.starts_with('=') && !is_page_break(t)
}

const HEADING_PREFIXES: [&str; 6] = ["INT./EXT", "INT/EXT", "I/E", "INT", "EXT", "EST"];

/// A scene heading: forced with a leading period, or one of the standard prefixes
/// followed by a period or a space, with a blank line after.
fn is_heading(t: &str, next_blank: bool) -> bool {
    if let Some(rest) = t.strip_prefix('.') {
        return !rest.starts_with('.') && !rest.trim().is_empty();
    }
    next_blank
        && HEADING_PREFIXES.iter().any(|p| {
            t.len() > p.len()
                && t.is_char_boundary(p.len())
                && t[..p.len()].eq_ignore_ascii_case(p)
                && matches!(t.as_bytes()[p.len()], b'.' | b' ')
        })
}

pub(crate) fn heading(t: &str) -> ImportedScene {
    let mut h = t.strip_prefix('.').unwrap_or(t).trim().to_string();

    let mut number = None;
    if h.ends_with('#') {
        if let Some(open) = h[..h.len() - 1].rfind('#') {
            let n = h[open + 1..h.len() - 1].trim();
            if !n.is_empty()
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-.".contains(c))
            {
                number = Some(n.to_string());
                h = h[..open].trim_end().to_string();
            }
        }
    }

    // Longest first, so `INT./EXT.` is not read as `INT.`.
    const KINDS: [(&str, IntExt); 8] = [
        ("INT./EXT", IntExt::IntExt),
        ("EXT./INT", IntExt::ExtInt),
        ("INT/EXT", IntExt::IntExt),
        ("EXT/INT", IntExt::ExtInt),
        ("I/E", IntExt::IntExt),
        ("INT", IntExt::Int),
        ("EXT", IntExt::Ext),
        // Establishing shots are exteriors. Engine check: confirm in the real app.
        ("EST", IntExt::Ext),
    ];
    let upper = h.to_ascii_uppercase();
    let (int_ext, rest) = KINDS
        .iter()
        .find(|(p, _)| {
            upper.starts_with(p)
                && (upper.len() == p.len() || upper[p.len()..].starts_with(['.', ' ']))
        })
        .map(|(p, k)| (*k, h[p.len()..].trim_start_matches(['.', ' ']).to_string()))
        // A forced heading without a prefix keeps its whole text as the set.
        .unwrap_or((IntExt::Unknown, h.clone()));

    let (set, time_of_day) = match rest.rsplit_once(" - ") {
        Some((s, tod)) => (s.trim().to_string(), tod.trim().to_string()),
        None => (rest.trim().to_string(), String::new()),
    };
    ImportedScene {
        number,
        heading: h,
        int_ext,
        set,
        time_of_day,
        characters: Vec::new(),
        page: 0,
        length: Eighths::ZERO,
        tags: Vec::new(),
    }
}

/// A character cue: forced with `@`, or all caps with at least one letter. Must have a
/// blank line before and dialogue right after (the caller checks both).
fn is_character(t: &str) -> bool {
    if t.starts_with('@') {
        return true;
    }
    if t.starts_with(['!', '>', '~']) {
        return false;
    }
    let name = t.split('(').next().unwrap_or(t);
    name.chars().any(|c| c.is_alphabetic()) && !name.chars().any(|c| c.is_lowercase())
}

/// Fountain's two-space line inside dialogue: the speech goes on after a blank line.
fn is_dialogue_gap(line: &str) -> bool {
    line.trim_end_matches('\r') == "  "
}

/// The name without `@`, extensions in parentheses or the dual-dialogue `^`.
pub(crate) fn character_name(t: &str) -> String {
    let t = t.strip_prefix('@').unwrap_or(t);
    let t = t.trim_end().trim_end_matches('^');
    t.split('(').next().unwrap_or(t).trim().to_string()
}

/// Lines a paragraph takes when word-wrapped at `width` characters.
fn wrapped(text: &str, width: usize) -> u32 {
    let mut lines = 1;
    let mut col = 0;
    for word in text.split_whitespace() {
        let len = word.chars().count();
        if col == 0 {
            col = len;
        } else if col + 1 + len <= width {
            col += 1 + len;
        } else {
            lines += 1;
            col = len;
        }
        // A word longer than the line breaks across lines.
        while col > width {
            lines += 1;
            col -= width;
        }
    }
    lines
}

/// Eighths at a line position on pages of `per_page` lines, rounded to the nearest
/// eighth, halves up. In 64 bits: a position past about 268 million lines overflowed 32
/// (decision 175).
fn eighths_at(pos: u32, per_page: u32) -> u64 {
    let lines = u64::from(per_page.max(1));
    (u64::from(pos) * 16 + lines) / (2 * lines)
}

/// A scene's length from its first line to the next scene's first line. Positions round
/// to eighths before subtracting, so lengths add up to the script's length. No scene is
/// shorter than 1/8 (decisions.md, entry 9).
pub(crate) fn length(start: u32, end: u32, per_page: u32) -> Eighths {
    // A scene is at least 1/8 and at most a sheet's limit (999 pages).
    let max = crate::model::MAX_SHEET_EIGHTHS;
    let eighths = eighths_at(end, per_page)
        .saturating_sub(eighths_at(start, per_page))
        .clamp(1, u64::from(max));
    Eighths(u32::try_from(eighths).unwrap_or(max))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KITCHEN: &str = "\
Title: The Lighthouse Keeper's Ledger
Credit: written by
Author: Invented Writer

INT. KITCHEN - NIGHT #1#

Rain on the window. MARGO OKAFOR (40s) counts coins.

MARGO
Forty. Forty-one.

DESMOND (O.S.)
(calling)
Is the lamp lit?

MARGO (CONT'D)
Not yet.

EXT. LIGHTHOUSE GALLERY - CONTINUOUS #1A#

Wind. DESMOND climbs.

DESMOND
I'll do it myself.

INT./EXT. TRUCK - DAY

They drive. Nobody speaks.

CUT TO:

.FLASHBACK

@McCLANE
Yippee.
";

    #[test]
    fn reads_title_headings_and_numbers() {
        let s = parse(KITCHEN);
        assert_eq!(s.title.as_deref(), Some("The Lighthouse Keeper's Ledger"));
        let h: Vec<_> = s.scenes.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(
            h,
            [
                "INT. KITCHEN - NIGHT",
                "EXT. LIGHTHOUSE GALLERY - CONTINUOUS",
                "INT./EXT. TRUCK - DAY",
                "FLASHBACK"
            ]
        );
        assert_eq!(s.scenes[0].number.as_deref(), Some("1"));
        assert_eq!(s.scenes[1].number.as_deref(), Some("1A"));
        assert_eq!(s.scenes[2].number, None);
    }

    #[test]
    fn splits_heading_parts() {
        let s = parse(KITCHEN);
        let a = &s.scenes[0];
        assert_eq!(
            (a.int_ext, a.set.as_str(), a.time_of_day.as_str()),
            (IntExt::Int, "KITCHEN", "NIGHT")
        );
        let c = &s.scenes[2];
        assert_eq!(
            (c.int_ext, c.set.as_str(), c.time_of_day.as_str()),
            (IntExt::IntExt, "TRUCK", "DAY")
        );
        let d = &s.scenes[3];
        assert_eq!(
            (d.int_ext, d.set.as_str(), d.time_of_day.as_str()),
            (IntExt::Unknown, "FLASHBACK", "")
        );
    }

    #[test]
    fn speaking_characters_only_without_extensions() {
        let s = parse(KITCHEN);
        // MARGO OKAFOR is named in action but does not speak under that name.
        assert_eq!(s.scenes[0].characters, ["MARGO", "DESMOND"]);
        assert_eq!(s.scenes[1].characters, ["DESMOND"]);
        // An all-caps transition with a blank line after is not a character.
        assert!(s.scenes[2].characters.is_empty());
        // Forced with @, mixed case kept.
        assert_eq!(s.scenes[3].characters, ["McCLANE"]);
    }

    #[test]
    fn a_two_space_line_keeps_a_speech_going() {
        // Eight second paragraphs, each one line as action (60 wide) and two as dialogue
        // (35 wide): kept as dialogue, the scene runs 8 lines longer.
        let para = "These words run past thirty-five but stay under sixty.";
        let mut two = String::from("INT. HUT - NIGHT\n\nMARA\nFirst part.\n");
        for _ in 0..8 {
            two.push_str(&format!("  \n{para}\n"));
        }
        two.push_str("\nDELACROIX ^\nAt once.\n\nINT. YARD - DAY\n\nWind.\n");
        let kept = parse(&two);
        assert_eq!(kept.scenes[0].characters, ["MARA", "DELACROIX"]);
        // The same with truly blank lines: each second paragraph is action.
        let split = parse(&two.replace("  \n", "\n"));
        assert_eq!(split.scenes[0].characters, kept.scenes[0].characters);
        let lines = |s: &ImportedScript| s.scenes[0].length.0 * LINES_PER_PAGE / 8;
        assert!(
            lines(&kept) > lines(&split),
            "{:?} vs {:?}",
            kept.scenes[0].length,
            split.scenes[0].length
        );
        // A line of three spaces is blank, as before.
        let three = parse(&two.replace("  \n", "   \n"));
        assert_eq!(three.scenes[0].length, split.scenes[0].length);
    }

    #[test]
    fn heading_needs_blank_lines_unless_forced() {
        let s = parse("Action.\nINT. HALL - DAY\n\nMore.\n\nEXT. YARD - DAY\nStill action.\n");
        assert!(s.scenes.is_empty());
        let s = parse("Action.\n\n.INSIDE THE WHALE\nDark.\n");
        assert_eq!(s.scenes.len(), 1);
        // Two periods start action, not a heading.
        assert!(parse("\n..and then\n").scenes.is_empty());
        // Lower case prefixes count.
        assert_eq!(parse("int. hall - day\n\n").scenes[0].int_ext, IntExt::Int);
        // INTERCOM is not a heading.
        assert!(parse("INTERCOM BUZZES\n\n").scenes.is_empty());
    }

    #[test]
    fn boneyard_and_notes_are_ignored() {
        let s = parse(
            "INT. HALL - DAY\n\n/* cut:\n\nGUARD\nHalt.\n*/\n\nPIP\nHi.[[ a note ]]\n\n[[\nlong note\n]]\n\nEXT. YARD - DAY\n\n",
        );
        assert_eq!(s.scenes.len(), 2);
        assert_eq!(s.scenes[0].characters, ["PIP"]);
    }

    #[test]
    fn sections_and_synopses_do_not_print() {
        let a = parse("INT. A - DAY\n\nOne line.\n\nINT. B - DAY\n\n");
        let b =
            parse("# Act one\n\nINT. A - DAY\n\n= she arrives\n\nOne line.\n\nINT. B - DAY\n\n");
        assert_eq!(a.scenes[0].length, b.scenes[0].length);
    }

    #[test]
    fn short_scenes_are_at_least_one_eighth() {
        let s = parse("INT. A - DAY\n\nINT. B - DAY\n\n");
        assert_eq!(s.scenes[0].length, Eighths(1));
        assert_eq!(s.scenes[1].length, Eighths(1));
    }

    #[test]
    fn a_full_page_of_action_is_one_page() {
        // Heading, blank, then 54 single-line paragraphs packed as one block = 56 lines.
        let mut text = String::from("INT. A - DAY\n\n");
        for _ in 0..54 {
            text.push_str("Short line.\n");
        }
        text.push_str("\nINT. B - DAY\n\nx\n");
        let s = parse(&text);
        assert_eq!(s.scenes[0].length, Eighths(8));
        assert_eq!(s.scenes[0].page, 1);
        assert_eq!(s.scenes[1].page, 2);
    }

    /// Decision 195: 56 lines a page by default, so an eighth is 7 lines; another count
    /// is the user's, held to 40 to 70.
    #[test]
    fn lines_per_page_default_to_56_and_can_be_set() {
        // A scene whose heading falls on line `n`: a heading, a blank, n - 4 lines of
        // action and a blank before it.
        let on_line = |n: usize| {
            let mut text = String::from("INT. A - DAY\n\n");
            text.push_str(&"Short line.\n".repeat(n - 4));
            text.push_str("\nINT. B - DAY\n\nx\n");
            text
        };
        // Line 56 is on page 1 by default and on page 2 at 55 lines a page.
        assert_eq!(parse(&on_line(56)).scenes[1].page, 1);
        assert_eq!(parse(&on_line(57)).scenes[1].page, 2);
        assert_eq!(parse_with(&on_line(56), 55).scenes[1].page, 2);
        // Seven lines are one eighth, 56 are a page.
        assert_eq!(parse(&on_line(8)).scenes[0].length, Eighths(1));
        assert_eq!(parse(&on_line(57)).scenes[0].length, Eighths(8));
        // 40 and 70 can be set; outside that the count is held to the range.
        assert_eq!(parse_with(&on_line(41), 40).scenes[1].page, 2);
        assert_eq!(parse_with(&on_line(70), 70).scenes[1].page, 1);
        assert_eq!(parse_with(&on_line(41), 10), parse_with(&on_line(41), 40));
        assert_eq!(parse_with(&on_line(71), 500), parse_with(&on_line(71), 70));
        assert_eq!(
            parse(&on_line(60)),
            parse_with(&on_line(60), LINES_PER_PAGE)
        );
    }

    #[test]
    fn forced_page_break_starts_a_new_page() {
        let s = parse("INT. A - DAY\n\nx\n\n===\n\nINT. B - DAY\n\nx\n");
        assert_eq!(s.scenes[1].page, 2);
        assert_eq!(s.scenes[0].length, Eighths(8));
    }

    #[test]
    fn wraps_words() {
        assert_eq!(wrapped("", 10), 1);
        assert_eq!(wrapped("aaaa bbbb", 9), 1);
        assert_eq!(wrapped("aaaa bbbbb", 9), 2);
        assert_eq!(wrapped(&"x".repeat(25), 10), 3);
    }

    #[test]
    fn handles_windows_line_endings() {
        let s = parse("INT. A - DAY\r\n\r\nBOB\r\nHi.\r\n");
        assert_eq!(s.scenes[0].characters, ["BOB"]);
    }

    #[test]
    fn property_random_scripts_parse_sanely() {
        use crate::testgen::Rng;
        let pieces = [
            "INT. ROOM - DAY",
            "EXT. FIELD - NIGHT #12#",
            ".FORCED",
            "Some action that runs on for a while and wraps past the edge of the column.",
            "BOB",
            "(quietly)",
            "Words, words.",
            "===",
            "/*",
            "*/",
            "[[",
            "]]",
            "",
            "",
            "CUT TO:",
            "# Section",
            "= synopsis",
            "@ann",
            "ÉCOLE",
        ];
        let mut rng = Rng::new(42);
        for _ in 0..500 {
            let n = rng.below(60) as usize;
            let text: Vec<&str> = (0..n)
                .map(|_| pieces[rng.below(pieces.len() as u64) as usize])
                .collect();
            let s = parse(&text.join("\n"));
            for w in s.scenes.windows(2) {
                assert!(w[0].page <= w[1].page);
            }
            for sc in &s.scenes {
                assert!(sc.length >= Eighths(1));
                assert!(sc.page >= 1);
            }
        }
    }
}
