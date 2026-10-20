//! Final Draft `.fdx` import: scene headings, page lengths and speaking characters.
//!
//! An `.fdx` file is XML with no public spec, so the shape we read comes from public
//! sample data and our own generator (docs/research/interop-formats.md, section 2). Like
//! the Fountain import, it brings in headings and the characters who speak (decisions.md,
//! entry 8). Unlike Fountain, every scene carries its own length, and we use it as written
//! (entries 9 and 12). Revisions, notes and styles are ignored. Production tags are read only when
//! asked for ([`parse_with_tags`]; decisions.md, IMP-04).
//!
//! The engine has no dependencies (entry 3), so a small strict XML reader lives here. It
//! refuses any DOCTYPE, which rules out DTDs and external entities, knows only the five
//! standard entities and numeric references, and caps the input size, the nesting depth,
//! the number of elements and the attributes on one element.

use std::fmt;

use crate::fountain::{self, ImportedScript, ImportedTag};
use crate::Eighths;

/// The largest file we read, in bytes.
pub const MAX_INPUT: usize = 64 * 1024 * 1024;
/// How deep elements may nest.
pub const MAX_DEPTH: usize = 256;
/// How many elements a file may hold. A feature script has tens of thousands.
pub const MAX_ELEMENTS: usize = 1_000_000;
/// How many attributes one element may carry.
pub const MAX_ATTRIBUTES: usize = 256;

/// Why a file could not be read, and the line where it went wrong (from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FdxError {
    pub line: usize,
    pub kind: FdxErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FdxErrorKind {
    /// The file is larger than `MAX_INPUT`.
    TooLarge,
    /// The file has a DOCTYPE. We never read DTDs.
    Doctype,
    /// Elements nest deeper than `MAX_DEPTH`.
    TooDeep,
    /// The file has more than `MAX_ELEMENTS` elements.
    TooManyElements,
    /// An element has more than `MAX_ATTRIBUTES` attributes.
    TooManyAttributes,
    /// An entity other than the five standard ones, or a bad character reference.
    BadEntity,
    /// A character XML does not allow, such as a control character.
    BadChar,
    /// The file stops in the middle of something, or with elements still open.
    UnexpectedEnd,
    /// A closing tag that does not match the open element.
    MismatchedTag,
    /// The outermost element is not `FinalDraft`.
    NotFinalDraft,
    /// Anything else that is not well-formed XML.
    Malformed(&'static str),
}

impl fmt::Display for FdxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match &self.kind {
            FdxErrorKind::TooLarge => "the file is too large to be a script",
            FdxErrorKind::Doctype => "the file has a DOCTYPE, which is not read for safety",
            FdxErrorKind::TooDeep => "elements nest too deeply",
            FdxErrorKind::TooManyElements => "the file has too many elements",
            FdxErrorKind::TooManyAttributes => "an element has too many attributes",
            FdxErrorKind::BadEntity => "unknown entity or bad character reference",
            FdxErrorKind::BadChar => "a character that is not allowed in XML",
            FdxErrorKind::UnexpectedEnd => "the file ends too soon",
            FdxErrorKind::MismatchedTag => "a closing tag does not match the open element",
            FdxErrorKind::NotFinalDraft => "this is not an .fdx script file",
            FdxErrorKind::Malformed(why) => why,
        };
        write!(f, "line {}: {what}", self.line)
    }
}

impl std::error::Error for FdxError {}

/// Reads a Final Draft file. Each scene heading paragraph starts a scene; its length and
/// page come from its scene properties, and a missing or unreadable length counts as 1/8.
/// A character counts as speaking when a dialogue paragraph follows its cue before the
/// next cue or heading. The title is the first line of the title page, if there is one.
pub fn parse(text: &str) -> Result<ImportedScript, FdxError> {
    read(text, false)
}

/// [`parse`], and production tags too: each scene gets the tagged items in its heading
/// and body as [`ImportedTag`]s (decisions.md, IMP-04).
///
/// No public sample shows how a tagged word links to its category, so this reads the
/// shape we expect and nothing else (engine check E-I5): a text run carries a
/// `TagNumber` attribute; after the content, a `TagData` block lists `TagCategory`
/// elements (`Id`, `Name`), `TagDefinition` elements (`Id`, `CatId`, `Label`) and `Tag`
/// elements (`Number`) whose `DefId` children name the definitions. The element is the
/// definition's label (else the tagged words), in the category's name. Runs whose tag
/// can't be followed to a category are left out.
pub fn parse_with_tags(text: &str) -> Result<ImportedScript, FdxError> {
    read(text, true)
}

fn read(text: &str, with_tags: bool) -> Result<ImportedScript, FdxError> {
    let mut r = Reader::new(text)?;
    let mut tags = TagData::default();
    let mut out = ImportedScript::default();
    // Open elements, outermost first.
    let mut names: Vec<&str> = Vec::new();
    // Open paragraphs, innermost last. Dual dialogue nests paragraphs inside one.
    let mut paras: Vec<Para> = Vec::new();
    // A cue waiting for its dialogue.
    let mut speaker: Option<String> = None;
    loop {
        match r.next()? {
            Event::Start { name, attrs } => {
                if names.is_empty() && name != "FinalDraft" {
                    return Err(r.error(FdxErrorKind::NotFinalDraft));
                }
                match name {
                    "Paragraph" => {
                        let place = match names.get(1) {
                            Some(&"Content") => Place::Body,
                            Some(&"TitlePage") => Place::Title,
                            _ => Place::Other,
                        };
                        paras.push(Para {
                            place,
                            kind: attr(&attrs, "Type"),
                            number: attr(&attrs, "Number"),
                            length: None,
                            page: None,
                            text: String::new(),
                            in_text: 0,
                            run_tag: None,
                        });
                    }
                    "Text" => {
                        if let Some(p) = paras.last_mut() {
                            p.in_text += 1;
                            if with_tags && p.place == Place::Body {
                                // A heading's tags go to the scene it starts.
                                let heading = p.kind.as_deref() == Some("Scene Heading");
                                let scene = out.scenes.len() + usize::from(heading);
                                p.run_tag = attr(&attrs, "TagNumber")
                                    .filter(|_| scene > 0)
                                    .map(|n| (scene - 1, n.trim().to_string(), String::new()));
                            }
                        }
                    }
                    "TagCategory" if with_tags => {
                        if let (Some(id), Some(name)) = (attr(&attrs, "Id"), attr(&attrs, "Name")) {
                            tags.categories.push((id, name));
                        }
                    }
                    "TagDefinition" if with_tags => {
                        if let (Some(id), Some(cat)) = (attr(&attrs, "Id"), attr(&attrs, "CatId")) {
                            let label = attr(&attrs, "Label").unwrap_or_default();
                            tags.definitions.push((id, cat, label));
                        }
                    }
                    "Tag" if with_tags => {
                        tags.open = attr(&attrs, "Number").map(|n| n.trim().to_string());
                    }
                    "DefId" if with_tags && tags.open.is_some() => tags.in_def = true,
                    "SceneProperties" if names.last() == Some(&"Paragraph") => {
                        if let Some(p) = paras.last_mut() {
                            p.length = attr(&attrs, "Length");
                            p.page = attr(&attrs, "Page");
                        }
                    }
                    _ => {}
                }
                names.push(name);
            }
            Event::End => match names.pop() {
                Some("Text") => {
                    if let Some(p) = paras.last_mut() {
                        p.in_text = p.in_text.saturating_sub(1);
                        if let Some(run) = p.run_tag.take() {
                            tags.runs.push(run);
                        }
                    }
                }
                Some("Tag") => tags.open = None,
                Some("DefId") => tags.in_def = false,
                Some("Paragraph") => {
                    if let Some(p) = paras.pop() {
                        finish(p, &mut out, &mut speaker);
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if tags.in_def {
                    if let Some(n) = &tags.open {
                        tags.links.push((n.clone(), t.trim().to_string()));
                    }
                }
                if let Some(p) = paras.last_mut() {
                    if p.in_text > 0 {
                        p.text.push_str(&t);
                        if let Some(run) = &mut p.run_tag {
                            run.2.push_str(&t);
                        }
                    }
                }
            }
            Event::Eof => break,
        }
    }
    if with_tags {
        tags.apply(&mut out);
    }
    Ok(out)
}

/// What the tag data block and the tagged runs said, gathered while reading.
#[derive(Default)]
struct TagData {
    /// Category id and name.
    categories: Vec<(String, String)>,
    /// Definition id, its category id and its label.
    definitions: Vec<(String, String, String)>,
    /// Tag number and a definition id it names.
    links: Vec<(String, String)>,
    /// Scene index, tag number and the run's words.
    runs: Vec<(usize, String, String)>,
    /// The `Tag` element being read, by number, and whether inside its `DefId`.
    open: Option<String>,
    in_def: bool,
}

impl TagData {
    fn apply(&self, out: &mut ImportedScript) {
        for (scene, number, words) in &self.runs {
            for (_, def) in self.links.iter().filter(|(n, _)| n == number) {
                let Some((_, cat, label)) = self.definitions.iter().find(|(id, ..)| id == def)
                else {
                    continue;
                };
                let Some((_, category)) = self.categories.iter().find(|(id, _)| id == cat) else {
                    continue;
                };
                let name = if label.trim().is_empty() {
                    words
                } else {
                    label
                };
                let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
                let category = category.trim().to_string();
                if name.is_empty() || category.is_empty() {
                    continue;
                }
                let Some(sc) = out.scenes.get_mut(*scene) else {
                    continue;
                };
                let tag = ImportedTag { category, name };
                let seen = sc.tags.iter().any(|t| {
                    t.category.eq_ignore_ascii_case(&tag.category)
                        && t.name.eq_ignore_ascii_case(&tag.name)
                });
                if !seen {
                    sc.tags.push(tag);
                }
            }
        }
    }
}

/// Where a paragraph sits: the script body, the title page, or somewhere we ignore
/// (headers, footers, notes).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Body,
    Title,
    Other,
}

struct Para {
    place: Place,
    kind: Option<String>,
    number: Option<String>,
    length: Option<String>,
    page: Option<String>,
    /// Every text run joined.
    text: String,
    /// Open `Text` elements inside this paragraph.
    in_text: u32,
    /// The open run's tag, when reading tags: scene index, tag number, the run's words.
    run_tag: Option<(usize, String, String)>,
}

fn attr(attrs: &[(&str, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.clone())
}

/// Applies a closed paragraph to the script.
fn finish(p: Para, out: &mut ImportedScript, speaker: &mut Option<String>) {
    let text = p.text.trim();
    match p.place {
        Place::Title => {
            if out.title.is_none() && !text.is_empty() {
                out.title = Some(text.to_string());
            }
        }
        Place::Other => {}
        Place::Body => match p.kind.as_deref() {
            Some("Scene Heading") => {
                *speaker = None;
                let mut scene = fountain::heading(text);
                if let Some(n) = p.number.as_deref().map(str::trim) {
                    if !n.is_empty() {
                        scene.number = Some(n.to_string());
                    }
                }
                // No length, or one we cannot read: 1/8, as the reference app showed.
                scene.length = p
                    .length
                    .as_deref()
                    .and_then(|l| Eighths::from_script(l).ok())
                    .map(|e| Eighths(e.0.min(crate::model::MAX_SHEET_EIGHTHS)))
                    .unwrap_or(Eighths(1));
                // No page: the previous scene's page, or the first.
                scene.page = p
                    .page
                    .as_deref()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .filter(|&n| n > 0)
                    .or(out.scenes.last().map(|s| s.page))
                    .unwrap_or(1);
                out.scenes.push(scene);
            }
            Some("Character") => {
                let name = fountain::character_name(text);
                *speaker = (!name.is_empty()).then_some(name);
            }
            Some("Dialogue") => {
                if let (Some(name), Some(scene)) = (speaker.take(), out.scenes.last_mut()) {
                    if !scene.characters.contains(&name) {
                        scene.characters.push(name);
                    }
                }
            }
            _ => {}
        },
    }
}

// ---- the XML reader

enum Event<'a> {
    Start {
        name: &'a str,
        attrs: Vec<(&'a str, String)>,
    },
    /// Closes the most recent open element. An empty element `<a/>` gives Start then End.
    End,
    /// Character data, with entities decoded and line endings made `\n`.
    Text(String),
    Eof,
}

/// A pull reader over a whole document. It never recurses, so depth cannot overflow the
/// stack, and every slice it takes ends on an ASCII byte, so it stays on char boundaries.
struct Reader<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    /// Where the token being read starts, for error lines.
    token: usize,
    open: Vec<&'a str>,
    elements: usize,
    seen_root: bool,
    /// An empty element was just started and still needs its End.
    pending_end: bool,
}

impl<'a> Reader<'a> {
    fn new(src: &'a str) -> Result<Reader<'a>, FdxError> {
        let mut r = Reader {
            src,
            b: src.as_bytes(),
            pos: 0,
            token: 0,
            open: Vec::new(),
            elements: 0,
            seen_root: false,
            pending_end: false,
        };
        if src.len() > MAX_INPUT {
            return Err(r.error(FdxErrorKind::TooLarge));
        }
        for (i, c) in src.char_indices() {
            let bad = (c < ' ' && !matches!(c, '\t' | '\n' | '\r'))
                || matches!(c, '\u{FFFE}' | '\u{FFFF}');
            if bad {
                return Err(r.error_at(i, FdxErrorKind::BadChar));
            }
        }
        // A byte order mark is allowed before the declaration.
        if src.starts_with('\u{FEFF}') {
            r.pos = '\u{FEFF}'.len_utf8();
        }
        Ok(r)
    }

    fn error(&self, kind: FdxErrorKind) -> FdxError {
        self.error_at(self.token, kind)
    }

    fn error_at(&self, pos: usize, kind: FdxErrorKind) -> FdxError {
        let end = pos.min(self.b.len());
        let line = self.b[..end].iter().filter(|&&b| b == b'\n').count() + 1;
        FdxError { line, kind }
    }

    fn rest(&self) -> &'a [u8] {
        &self.b[self.pos..]
    }

    fn at(&self, s: &str) -> bool {
        self.rest().starts_with(s.as_bytes())
    }

    /// Position of `s` at or after `from`, if any.
    fn find(&self, from: usize, s: &str) -> Option<usize> {
        self.src.get(from..)?.find(s).map(|i| from + i)
    }

    fn skip_ws(&mut self) -> bool {
        let start = self.pos;
        while self.pos < self.b.len() && is_ws(self.b[self.pos]) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn next(&mut self) -> Result<Event<'a>, FdxError> {
        if self.pending_end {
            self.pending_end = false;
            self.open.pop();
            return Ok(Event::End);
        }
        loop {
            self.token = self.pos;
            if self.pos >= self.b.len() {
                if !self.open.is_empty() {
                    return Err(self.error(FdxErrorKind::UnexpectedEnd));
                }
                if !self.seen_root {
                    return Err(self.error(FdxErrorKind::Malformed("no root element")));
                }
                return Ok(Event::Eof);
            }
            if self.b[self.pos] != b'<' {
                let end = self.find(self.pos, "<").unwrap_or(self.b.len());
                let raw = &self.src[self.pos..end];
                if self.open.is_empty() {
                    if !raw.bytes().all(is_ws) {
                        return Err(
                            self.error(FdxErrorKind::Malformed("text outside the root element"))
                        );
                    }
                    self.pos = end;
                    continue;
                }
                let text = decode(raw, false).map_err(|(i, k)| self.error_at(self.pos + i, k))?;
                self.pos = end;
                return Ok(Event::Text(text));
            }
            if self.at("<?") {
                self.instruction()?;
            } else if self.at("<!--") {
                let end = self
                    .find(self.pos + 4, "-->")
                    .ok_or_else(|| self.error(FdxErrorKind::UnexpectedEnd))?;
                self.pos = end + 3;
            } else if self.at("<![CDATA[") {
                if self.open.is_empty() {
                    return Err(
                        self.error(FdxErrorKind::Malformed("CDATA outside the root element"))
                    );
                }
                let start = self.pos + 9;
                let end = self
                    .find(start, "]]>")
                    .ok_or_else(|| self.error(FdxErrorKind::UnexpectedEnd))?;
                let text = self.src[start..end]
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");
                self.pos = end + 3;
                return Ok(Event::Text(text));
            } else if self.at("<!DOCTYPE") {
                return Err(self.error(FdxErrorKind::Doctype));
            } else if self.at("<!") {
                return Err(self.error(FdxErrorKind::Malformed("unknown markup after <!")));
            } else if self.at("</") {
                return self.end_tag();
            } else {
                return self.start_tag();
            }
        }
    }

    /// The XML declaration, allowed only at the very start, or a processing instruction.
    fn instruction(&mut self) -> Result<(), FdxError> {
        self.pos += 2;
        let target = self.name()?;
        let at_start = self.token == 0 || self.src[..self.token] == *"\u{FEFF}";
        if target.eq_ignore_ascii_case("xml") && !(target == "xml" && at_start) {
            return Err(self.error(FdxErrorKind::Malformed(
                "the XML declaration must come first",
            )));
        }
        if !self.skip_ws() && !self.at("?>") {
            return Err(self.error(FdxErrorKind::Malformed("bad processing instruction")));
        }
        let end = self
            .find(self.pos, "?>")
            .ok_or_else(|| self.error(FdxErrorKind::UnexpectedEnd))?;
        self.pos = end + 2;
        Ok(())
    }

    fn start_tag(&mut self) -> Result<Event<'a>, FdxError> {
        self.pos += 1;
        let name = self.name()?;
        let mut attrs: Vec<(&'a str, String)> = Vec::new();
        let empty = loop {
            let spaced = self.skip_ws();
            if self.at(">") {
                self.pos += 1;
                break false;
            }
            if self.at("/>") {
                self.pos += 2;
                break true;
            }
            if self.pos >= self.b.len() {
                return Err(self.error(FdxErrorKind::UnexpectedEnd));
            }
            if !spaced {
                return Err(self.error(FdxErrorKind::Malformed("expected a space or >")));
            }
            let key = self.name()?;
            self.skip_ws();
            if !self.at("=") {
                return Err(self.error(FdxErrorKind::Malformed("expected = after a name")));
            }
            self.pos += 1;
            self.skip_ws();
            let quote = match self.rest().first() {
                Some(b'"') => "\"",
                Some(b'\'') => "'",
                Some(_) => {
                    return Err(self.error(FdxErrorKind::Malformed("expected a quoted value")))
                }
                None => return Err(self.error(FdxErrorKind::UnexpectedEnd)),
            };
            let start = self.pos + 1;
            let end = self
                .find(start, quote)
                .ok_or_else(|| self.error(FdxErrorKind::UnexpectedEnd))?;
            let raw = &self.src[start..end];
            if let Some(i) = raw.find('<') {
                return Err(self.error_at(start + i, FdxErrorKind::Malformed("< in a value")));
            }
            let value = decode(raw, true).map_err(|(i, k)| self.error_at(start + i, k))?;
            self.pos = end + 1;
            if attrs.iter().any(|(k, _)| *k == key) {
                return Err(self.error(FdxErrorKind::Malformed("an attribute is repeated")));
            }
            if attrs.len() >= MAX_ATTRIBUTES {
                return Err(self.error(FdxErrorKind::TooManyAttributes));
            }
            attrs.push((key, value));
        };
        if self.open.is_empty() && self.seen_root {
            return Err(self.error(FdxErrorKind::Malformed("more than one root element")));
        }
        if self.open.len() >= MAX_DEPTH {
            return Err(self.error(FdxErrorKind::TooDeep));
        }
        self.elements += 1;
        if self.elements > MAX_ELEMENTS {
            return Err(self.error(FdxErrorKind::TooManyElements));
        }
        self.seen_root = true;
        self.open.push(name);
        self.pending_end = empty;
        Ok(Event::Start { name, attrs })
    }

    fn end_tag(&mut self) -> Result<Event<'a>, FdxError> {
        self.pos += 2;
        let name = self.name()?;
        self.skip_ws();
        if !self.at(">") {
            return Err(if self.pos >= self.b.len() {
                self.error(FdxErrorKind::UnexpectedEnd)
            } else {
                self.error(FdxErrorKind::Malformed("expected > to end a closing tag"))
            });
        }
        self.pos += 1;
        match self.open.pop() {
            Some(open) if open == name => Ok(Event::End),
            _ => Err(self.error(FdxErrorKind::MismatchedTag)),
        }
    }

    /// An XML name. Any non-ASCII character is accepted as a name character.
    fn name(&mut self) -> Result<&'a str, FdxError> {
        let start = self.pos;
        let first = |b: u8| b.is_ascii_alphabetic() || b == b'_' || b == b':' || b >= 0x80;
        let more = |b: u8| first(b) || b.is_ascii_digit() || b == b'-' || b == b'.';
        match self.rest().first() {
            Some(&b) if first(b) => self.pos += 1,
            Some(_) => return Err(self.error(FdxErrorKind::Malformed("expected a name"))),
            None => return Err(self.error(FdxErrorKind::UnexpectedEnd)),
        }
        while self.pos < self.b.len() && more(self.b[self.pos]) {
            self.pos += 1;
        }
        Ok(&self.src[start..self.pos])
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Decodes entities and character references and makes line endings `\n`. In an
/// attribute value, literal tabs and line endings become spaces, as XML says. On failure,
/// gives the byte offset into `raw`.
fn decode(raw: &str, attribute: bool) -> Result<String, (usize, FdxErrorKind)> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '&' => {
                let rest = &raw[i + 1..];
                let semi = rest
                    .find(';')
                    .filter(|&n| n <= 10)
                    .ok_or((i, FdxErrorKind::BadEntity))?;
                let ent = &rest[..semi];
                out.push(entity(ent).ok_or((i, FdxErrorKind::BadEntity))?);
                // Skip the entity body and the semicolon, all ASCII if it decoded.
                for _ in 0..=semi {
                    chars.next();
                }
            }
            '\r' => {
                if chars.peek().is_some_and(|&(_, n)| n == '\n') {
                    chars.next();
                }
                out.push(if attribute { ' ' } else { '\n' });
            }
            '\n' | '\t' if attribute => out.push(' '),
            '>' if !attribute && raw[..i].ends_with("]]") => {
                return Err((i, FdxErrorKind::Malformed("]]> in text")));
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let (digits, radix) = match name.strip_prefix("#x") {
                Some(h) => (h, 16),
                None => (name.strip_prefix('#')?, 10),
            };
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return None;
            }
            let c = char::from_u32(u32::from_str_radix(digits, radix).ok()?)?;
            let allowed = matches!(c, '\t' | '\n' | '\r')
                || (c >= ' ' && !matches!(c, '\u{FFFE}' | '\u{FFFF}'));
            allowed.then_some(c)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fountain::IntExt;
    use crate::Schedule;

    /// An invented two-scene script with a title page, a parenthetical, dual dialogue and
    /// blocks after the content that the import ignores.
    const HARBOR: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="5">
<Content>
<Paragraph Number="1" Type="Scene Heading">
<SceneProperties Length="1 3/8" Page="1" Title=""/>
<Text>INT. HARBOR OFFICE - NIGHT</Text>
</Paragraph>
<Paragraph Type="Action">
<Text>TOVA WREN sorts tide tables. RUFUS naps.</Text>
</Paragraph>
<Paragraph Type="Character">
<Text>TOVA</Text>
</Paragraph>
<Paragraph Type="Parenthetical">
<Text>(quietly)</Text>
</Paragraph>
<Paragraph Type="Dialogue">
<Text>High water at four.</Text>
</Paragraph>
<Paragraph Type="Character">
<Text>PELL (V.O.)</Text>
</Paragraph>
<Paragraph Type="Dialogue">
<Text>Then we sail at five.</Text>
</Paragraph>
<Paragraph Type="Character">
<Text>TOVA (CONT'D)</Text>
</Paragraph>
<Paragraph Type="Dialogue">
<Text>We do not.</Text>
</Paragraph>
<Paragraph Number="2A" Type="Scene Heading">
<SceneProperties Length="4/8" Page="2" Title=""/>
<Text>EXT. PIER - DAY</Text>
</Paragraph>
<Paragraph Type="General">
<DualDialogue>
<Paragraph Type="Character">
<Text>PELL</Text>
</Paragraph>
<Paragraph Type="Dialogue">
<Text>Now!</Text>
</Paragraph>
<Paragraph Type="Character">
<Text>MOSS ^</Text>
</Paragraph>
<Paragraph Type="Dialogue">
<Text>Not now!</Text>
</Paragraph>
</DualDialogue>
</Paragraph>
<Paragraph Type="Shot">
<Text>ANGLE ON THE ROPE</Text>
</Paragraph>
</Content>
<HeaderAndFooter>
<Header><Paragraph Type="Header"><Text>NOT A SCENE</Text></Paragraph></Header>
</HeaderAndFooter>
<TitlePage>
<Content>
<Paragraph Alignment="Center"><Text></Text></Paragraph>
<Paragraph Alignment="Center"><Text>Tide </Text><Text Style="Bold">Tables</Text></Paragraph>
<Paragraph Alignment="Center"><Text>Written by an invented writer</Text></Paragraph>
</Content>
</TitlePage>
<!-- a comment -->
</FinalDraft>
"#;

    /// A body of paragraphs wrapped as a file.
    fn wrap(body: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<FinalDraft Version=\"5\"><Content>\n\
             {body}\n</Content></FinalDraft>\n"
        )
    }

    fn heading(n: u32, length: &str, text: &str) -> String {
        format!(
            "<Paragraph Number=\"{n}\" Type=\"Scene Heading\">\
             <SceneProperties Length=\"{length}\" Page=\"{n}\"/><Text>{text}</Text></Paragraph>\n"
        )
    }

    fn para(kind: &str, text: &str) -> String {
        format!("<Paragraph Type=\"{kind}\"><Text>{text}</Text></Paragraph>\n")
    }

    fn kind(text: &str) -> FdxErrorKind {
        parse(text).unwrap_err().kind
    }

    #[test]
    fn reads_a_normal_script() {
        let s = parse(HARBOR).unwrap();
        assert_eq!(s.title.as_deref(), Some("Tide Tables"));
        assert_eq!(s.scenes.len(), 2);
        let a = &s.scenes[0];
        assert_eq!(a.number.as_deref(), Some("1"));
        assert_eq!(a.heading, "INT. HARBOR OFFICE - NIGHT");
        assert_eq!(
            (a.int_ext, a.set.as_str(), a.time_of_day.as_str()),
            (IntExt::Int, "HARBOR OFFICE", "NIGHT")
        );
        assert_eq!((a.page, a.length), (1, Eighths(11)));
        // Named only in action: not added. Extensions dropped, first-seen order, no repeats.
        assert_eq!(a.characters, ["TOVA", "PELL"]);
        let b = &s.scenes[1];
        assert_eq!(b.number.as_deref(), Some("2A"));
        assert_eq!((b.int_ext, b.set.as_str()), (IntExt::Ext, "PIER"));
        assert_eq!((b.page, b.length), (2, Eighths(4)));
        // Dual dialogue sits inside a general paragraph; both cues count, ^ dropped.
        assert_eq!(b.characters, ["PELL", "MOSS"]);
    }

    #[test]
    fn reads_every_length_form() {
        let forms = [
            ("8/8", 8),
            ("1 0/8", 8),
            ("1", 1),
            ("10/8", 10),
            ("2 1/8", 17),
        ];
        let body: String = forms
            .iter()
            .enumerate()
            .map(|(i, (l, _))| heading(i as u32 + 1, l, "INT. ROOM - DAY"))
            .collect();
        let s = parse(&wrap(&body)).unwrap();
        let got: Vec<u32> = s.scenes.iter().map(|s| s.length.0).collect();
        let want: Vec<u32> = forms.iter().map(|(_, n)| *n).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn missing_or_unreadable_length_is_one_eighth() {
        let body = "<Paragraph Type=\"Scene Heading\"><Text>INT. A - DAY</Text></Paragraph>\n\
                    <Paragraph Type=\"Scene Heading\"><SceneProperties Length=\"half\" Page=\"3\"/>\
                    <Text>INT. B - DAY</Text></Paragraph>";
        let s = parse(&wrap(body)).unwrap();
        assert_eq!(s.scenes[0].length, Eighths(1));
        assert_eq!(s.scenes[1].length, Eighths(1));
        // No number attribute: no number. No page: the first page.
        assert_eq!(s.scenes[0].number, None);
        assert_eq!((s.scenes[0].page, s.scenes[1].page), (1, 3));
        assert_eq!(s.title, None);
    }

    #[test]
    fn joins_text_runs() {
        let body = "<Paragraph Type=\"Scene Heading\"><SceneProperties Length=\"2/8\"/>\
                    <Text>  EXT. </Text><Text Style=\"Bold\">ORCHARD</Text>\
                    <Text> - DUSK  </Text></Paragraph>\n\
                    <Paragraph Type=\"Character\"><Text>JUN</Text><Text>O</Text></Paragraph>\
                    <Paragraph Type=\"Dialogue\"><Text>Apples.</Text></Paragraph>";
        let s = parse(&wrap(body)).unwrap();
        assert_eq!(s.scenes[0].heading, "EXT. ORCHARD - DUSK");
        assert_eq!(s.scenes[0].time_of_day, "DUSK");
        assert_eq!(s.scenes[0].characters, ["JUNO"]);
    }

    #[test]
    fn decodes_entities_cdata_and_references() {
        let body = heading(1, "1/8", "INT. BAKER &amp; SONS&#39; SHOP - DAY")
            + &para("Character", "B&#x00C9;A")
            + &para("Dialogue", "&lt;hi&gt; &quot;there&quot; &apos;you&apos;")
            + "<Paragraph Type=\"Character\"><Text><![CDATA[R&D <BOT>]]></Text></Paragraph>"
            + &para("Dialogue", "Beep.");
        let s = parse(&wrap(&body)).unwrap();
        assert_eq!(s.scenes[0].set, "BAKER & SONS' SHOP");
        assert_eq!(s.scenes[0].characters, ["BÉA", "R&D <BOT>"]);
    }

    #[test]
    fn a_cue_without_dialogue_is_not_added() {
        let body = heading(1, "1", "INT. HALL - DAY")
            + &para("Character", "SILENT")
            + &para("Action", "Nothing is said.")
            + &para("Character", "TALKER")
            + &para("Parenthetical", "(beat)")
            + &para("Dialogue", "Hello.")
            + &para("Character", "LATE")
            + &heading(2, "1", "INT. YARD - DAY")
            + &para("Dialogue", "Orphan line.");
        let s = parse(&wrap(&body)).unwrap();
        assert_eq!(s.scenes[0].characters, ["TALKER"]);
        assert!(s.scenes[1].characters.is_empty());
    }

    #[test]
    fn refuses_a_doctype() {
        let text = "<?xml version=\"1.0\"?>\n<!DOCTYPE FinalDraft [<!ENTITY x SYSTEM \
                    \"file:///etc/passwd\">]>\n<FinalDraft><Content/></FinalDraft>";
        assert_eq!(
            parse(text),
            Err(FdxError {
                line: 2,
                kind: FdxErrorKind::Doctype
            })
        );
        // Unknown entities are refused too, with no DTD to define them.
        let text = wrap(&para("Action", "&x;"));
        assert_eq!(kind(&text), FdxErrorKind::BadEntity);
    }

    #[test]
    fn refuses_deep_nesting() {
        let deep = MAX_DEPTH + 10;
        let text = format!(
            "<FinalDraft>{}{}</FinalDraft>",
            "<a>".repeat(deep),
            "</a>".repeat(deep)
        );
        assert_eq!(kind(&text), FdxErrorKind::TooDeep);
        // Just inside the limit is fine.
        let ok = MAX_DEPTH - 1;
        let text = format!(
            "<FinalDraft>{}{}</FinalDraft>",
            "<a>".repeat(ok),
            "</a>".repeat(ok)
        );
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn refuses_too_many_elements_and_too_much_input() {
        let text = format!("<FinalDraft>{}</FinalDraft>", "<a/>".repeat(MAX_ELEMENTS));
        assert_eq!(kind(&text), FdxErrorKind::TooManyElements);
        let text = " ".repeat(MAX_INPUT + 1);
        assert_eq!(kind(&text), FdxErrorKind::TooLarge);
    }

    #[test]
    fn truncated_files_give_an_error_with_a_line() {
        for cut in [10, 80, 300, 700, HARBOR.len() - 3] {
            let mut cut = cut;
            while !HARBOR.is_char_boundary(cut) {
                cut -= 1;
            }
            let e = parse(&HARBOR[..cut]).unwrap_err();
            assert!(
                e.line >= 1 && e.line <= HARBOR[..cut].lines().count() + 1,
                "{e}"
            );
        }
        assert_eq!(
            parse("<FinalDraft>\n<Content>\n<Paragraph"),
            Err(FdxError {
                line: 3,
                kind: FdxErrorKind::UnexpectedEnd
            })
        );
    }

    #[test]
    fn rejects_malformed_xml() {
        use FdxErrorKind::*;
        for (text, want) in [
            ("", Malformed("no root element")),
            ("<Script/>", NotFinalDraft),
            ("<FinalDraft></Content>", MismatchedTag),
            (
                "<FinalDraft/><FinalDraft/>",
                Malformed("more than one root element"),
            ),
            (
                "hello<FinalDraft/>",
                Malformed("text outside the root element"),
            ),
            (
                "<FinalDraft a=\"1\" a=\"2\"/>",
                Malformed("an attribute is repeated"),
            ),
            ("<FinalDraft a=1/>", Malformed("expected a quoted value")),
            ("<FinalDraft a=\"<\"/>", Malformed("< in a value")),
            ("<FinalDraft>&#0;</FinalDraft>", BadEntity),
            ("<FinalDraft>&#xD800;</FinalDraft>", BadEntity),
            ("<FinalDraft>&#+65;</FinalDraft>", BadEntity),
            ("<FinalDraft>\u{1}</FinalDraft>", BadChar),
            ("<FinalDraft>]]></FinalDraft>", Malformed("]]> in text")),
            (
                "<FinalDraft/>\n<?xml version=\"1.0\"?>",
                Malformed("the XML declaration must come first"),
            ),
            (
                "<FinalDraft><!ELEMENT x></FinalDraft>",
                Malformed("unknown markup after <!"),
            ),
        ] {
            assert_eq!(kind(text), want, "{text:?}");
        }
        // A byte order mark, processing instructions and comments are fine.
        assert!(
            parse("\u{FEFF}<?xml version=\"1.0\"?><?app hint?><!-- c --><FinalDraft/>").is_ok()
        );
    }

    #[test]
    fn errors_name_the_line() {
        let e = parse("<FinalDraft>\n<Content>\n\n<Text>&nope;</Text>\n</Content>\n</FinalDraft>")
            .unwrap_err();
        assert_eq!(e.line, 4);
        assert_eq!(
            e.to_string(),
            "line 4: unknown entity or bad character reference"
        );
    }

    /// A tagged script in the shape `parse_with_tags` reads (engine check E-I5).
    pub(crate) const TAGGED: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="5">
<Content>
<Paragraph Type="Scene Heading" Number="1"><SceneProperties Length="1" Page="1"/><Text>INT. </Text><Text TagNumber="3">BOATHOUSE</Text><Text> - NIGHT</Text></Paragraph>
<Paragraph Type="Action"><Text>She lifts the </Text><Text TagNumber="1">old lantern</Text><Text> and the </Text><Text TagNumber="2">rope</Text><Text>.</Text></Paragraph>
<Paragraph Type="Character"><Text>MARA</Text></Paragraph>
<Paragraph Type="Dialogue"><Text>Still works.</Text></Paragraph>
<Paragraph Type="Scene Heading" Number="2"><SceneProperties Length="2/8" Page="2"/><Text>EXT. PIER - DAY</Text></Paragraph>
<Paragraph Type="Action"><Text TagNumber="1">The lantern</Text><Text TagNumber="9">x</Text><Text TagNumber="4">Mara</Text></Paragraph>
</Content>
<TagData>
<TagCategories>
<TagCategory Id="c1" Name="Props" Number="7"/>
<TagCategory Id="c2" Name="Set Dressing" Number="8"/>
<TagCategory Id="c3" Name="Cast Members" Number="1"/>
</TagCategories>
<TagDefinitions>
<TagDefinition CatId="c1" Id="d1" Label="LANTERN"/>
<TagDefinition CatId="c2" Id="d2" Label=""/>
<TagDefinition CatId="c2" Id="d3" Label="Boathouse"/>
<TagDefinition CatId="c3" Id="d4" Label="MARA"/>
</TagDefinitions>
<Tags>
<Tag Number="1"><DefId>d1</DefId></Tag>
<Tag Number="2"><DefId>d2</DefId></Tag>
<Tag Number="3"><DefId>d3</DefId></Tag>
<Tag Number="4"><DefId>d4</DefId></Tag>
</Tags>
</TagData>
</FinalDraft>
"#;

    #[test]
    fn reads_production_tags_only_when_asked() {
        let plain = parse(TAGGED).unwrap();
        assert!(plain.scenes.iter().all(|s| s.tags.is_empty()));
        let s = parse_with_tags(TAGGED).unwrap();
        assert_eq!(s.scenes.len(), 2);
        let t = |i: usize| -> Vec<(String, String)> {
            s.scenes[i]
                .tags
                .iter()
                .map(|t| (t.category.clone(), t.name.clone()))
                .collect()
        };
        assert_eq!(
            t(0),
            vec![
                ("Set Dressing".to_string(), "Boathouse".to_string()),
                ("Props".to_string(), "LANTERN".to_string()),
                ("Set Dressing".to_string(), "rope".to_string()),
            ]
        );
        // Tag 9 has no definition: left out.
        assert_eq!(
            t(1),
            vec![
                ("Props".to_string(), "LANTERN".to_string()),
                ("Cast Members".to_string(), "MARA".to_string()),
            ]
        );
        assert_eq!(s.scenes[0].set, "BOATHOUSE");
        assert_eq!(s.scenes[0].characters, vec!["MARA".to_string()]);
    }

    #[test]
    fn imports_into_a_schedule() {
        let script = parse(HARBOR).unwrap();
        let mut s = Schedule::new("Tide Tables");
        let cast = s.category_named("Cast").unwrap();
        let sheets = s.import_script(&script, cast).unwrap();
        assert_eq!(sheets.len(), 2);
        s.check().unwrap();
    }

    #[test]
    fn property_mutated_files_never_panic() {
        use crate::testgen::Rng;
        let base = HARBOR.as_bytes();
        let snippets: [&[u8]; 12] = [
            b"<",
            b">",
            b"/>",
            b"&",
            b"&amp;",
            b"&#x;",
            b"\"",
            b"]]>",
            b"<!--",
            b"<![CDATA[",
            b"</Paragraph>",
            b"\xC3",
        ];
        let mut rng = Rng::new(0xfd_c0de);
        for _ in 0..3000 {
            let mut bytes = base.to_vec();
            for _ in 0..=rng.below(4) {
                let len = bytes.len() as u64;
                let at = rng.below(len + 1) as usize;
                match rng.below(6) {
                    0 if at < bytes.len() => bytes[at] = rng.next_u64() as u8,
                    1 => {
                        let end = (at + rng.below(40) as usize).min(bytes.len());
                        bytes.drain(at..end);
                    }
                    2 => {
                        let s = snippets[rng.below(snippets.len() as u64) as usize];
                        bytes.splice(at..at, s.iter().copied());
                    }
                    3 => bytes.truncate(at),
                    4 => {
                        let end = (at + rng.below(200) as usize).min(bytes.len());
                        let copy = bytes[at..end].to_vec();
                        bytes.splice(at..at, copy);
                    }
                    _ => {
                        let n = rng.below(4) as usize;
                        for _ in 0..n {
                            bytes.insert(at.min(bytes.len()), rng.next_u64() as u8);
                        }
                    }
                }
            }
            let text = String::from_utf8_lossy(&bytes);
            if let Ok(s) = parse(&text) {
                for sc in &s.scenes {
                    assert!(sc.page >= 1);
                }
            }
        }
    }
}
