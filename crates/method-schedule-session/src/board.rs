//! The board edits, as plain functions on a [`Schedule`]: moving strips, day breaks,
//! banners, sorting, and boards and sub-boards themselves. The page names the board and
//! sub-board it shows by id (sub-board ids are unique across the schedule, so a sub-board
//! names its board). Each function validates what the page sent; `Session::edit` makes it
//! one undo step.

use serde::Deserialize;

use method_schedule_core::days::{
    auto_day_breaks as engine_auto_breaks, remove_day_breaks as engine_remove_breaks, Limit,
};
use method_schedule_core::model::{
    Align, Banner, BannerColor, BannerFont, BannerStyle, BoardId, Error, Place, SortField, SortKey,
    StripColumn, StripField, StripId, StripKind, StripLayout, StripWidth, SubBoardId, TextSize,
    MAX_COLUMN_TEXT, MAX_STRIP_LINES,
};
use method_schedule_core::{Eighths, Schedule};

use crate::sheet::{duration_say, parse_duration, MAX_NAME};

pub type EditResult<T = ()> = Result<T, String>;

/// The longest banner text, in characters.
pub const MAX_BANNER: usize = 200;
pub use method_schedule_core::model::MAX_SORT_KEYS;

/// The first board's id.
pub fn first_board(s: &Schedule) -> EditResult<BoardId> {
    s.boards
        .first()
        .map(|b| b.id)
        .ok_or_else(|| "The schedule has no board.".to_string())
}

/// A board by id.
pub fn board_id(s: &Schedule, id: u32) -> EditResult<BoardId> {
    s.board(BoardId(id))
        .map(|b| b.id)
        .ok_or_else(|| "That board is no longer in the schedule.".to_string())
}

/// A sub-board by id, with the board it is on.
pub fn sub_board(s: &Schedule, id: u32) -> EditResult<(BoardId, SubBoardId)> {
    let sub = SubBoardId(id);
    s.board_of_sub(sub)
        .map(|b| (b, sub))
        .ok_or_else(|| "That sub-board is no longer in the schedule.".to_string())
}

/// The engine's refusals for boards, as the page says them.
pub fn say(e: Error) -> String {
    match e {
        Error::NoSuchBoard(_) => "That board is no longer in the schedule.".into(),
        Error::NoSuchSubBoard(_) => "That sub-board is no longer in the schedule.".into(),
        Error::NoSuchStrip(_) => "That strip is no longer on the board.".into(),
        Error::EmptyName => "Type a name.".into(),
        other => crate::elements::say(other),
    }
}

fn kind_of(s: &Schedule, board: BoardId, strip: StripId) -> Option<&StripKind> {
    let b = s.board(board)?;
    b.boneyard
        .iter()
        .chain(b.sub_boards.iter().flat_map(|x| &x.strips))
        .find(|x| x.id == strip)
        .map(|x| &x.kind)
}

/// A place on `board`: one of its sub-boards, or its Boneyard when `sub` is `None`.
fn place_on(s: &Schedule, board: BoardId, sub: Option<u32>, index: usize) -> EditResult<Place> {
    let sub_board = match sub {
        None => None,
        Some(id) => {
            let (b, sb) = sub_board(s, id)?;
            if b != board {
                return Err("That sub-board is on another board.".to_string());
            }
            Some(sb)
        }
    };
    Ok(Place { sub_board, index })
}

/// Moves strips of `board` to `index` in one of its sub-boards, or in its Boneyard when
/// `sub` is `None`. `index` counts positions before the move (`Schedule::move_strips`);
/// the strips keep their board order. Scenes and day breaks go in the Boneyard (a day
/// break there is parked: no day, no date, decisions.md entry 57); banners stay on the
/// sub-boards.
pub fn move_strips(
    s: &mut Schedule,
    board: u32,
    strips: &[u32],
    sub: Option<u32>,
    index: usize,
) -> EditResult {
    let board = board_id(s, board)?;
    if strips.is_empty() {
        return Ok(());
    }
    let to = place_on(s, board, sub, index)?;
    let ids: Vec<StripId> = strips.iter().map(|&i| StripId(i)).collect();
    for id in &ids {
        match kind_of(s, board, *id) {
            None => return Err("That strip is no longer on the board.".to_string()),
            Some(StripKind::Banner(_)) if to.sub_board.is_none() => {
                return Err("Banners go on a sub-board, not in the Boneyard.".to_string())
            }
            Some(StripKind::Sheet(sheet))
                if to.sub_board.is_none() && s.sheet_strips(board, *sheet).len() > 1 =>
            {
                return Err(
                    "A scene spanning days stays on a sub-board. Delete its other parts first."
                        .to_string(),
                )
            }
            Some(_) => {}
        }
    }
    s.move_strips(board, &ids, to).map_err(say)
}

/// Inserts a day break at `index` in a sub-board. Returns the new strip's id.
pub fn insert_day_break(s: &mut Schedule, sub: u32, index: usize) -> EditResult<u32> {
    let (board, sb) = sub_board(s, sub)?;
    let at = Place {
        sub_board: Some(sb),
        index,
    };
    s.insert_strip(board, StripKind::DayBreak, at)
        .map(|id| id.0)
        .map_err(say)
}

/// Deletes strips of `board`: day breaks and banners from where they are, and a scene's
/// sheet from every board (its strip on each, as Delete Sheet does; its elements stay in
/// the schedule). The page asks first about scenes and banners (inventory section 5,
/// decisions.md, entry 54). One undo step for the lot; an unknown strip refuses it all.
pub fn delete_strips(s: &mut Schedule, board: u32, strips: &[u32]) -> EditResult {
    let board = board_id(s, board)?;
    let mut kinds = Vec::with_capacity(strips.len());
    for &strip in strips {
        let id = StripId(strip);
        match kind_of(s, board, id) {
            Some(k) => kinds.push((id, k.clone())),
            None => return Err("That strip is no longer on the board.".to_string()),
        }
    }
    for (id, kind) in kinds {
        match kind {
            // One part of a scene spanning days goes alone; its last strip takes the sheet
            // (decisions.md, entry 76).
            StripKind::Sheet(sheet) if s.sheet_strips(board, sheet).len() > 1 => {
                s.delete_strip(board, id).map_err(say)?
            }
            StripKind::Sheet(sheet) => {
                if s.sheet(sheet).is_some() {
                    s.delete_sheet(sheet).map_err(say)?
                }
            }
            _ => s.delete_strip(board, id).map_err(say)?,
        }
    }
    Ok(())
}

/// Continue Scene on Next Day (decisions.md, entry 76): another part of the scene whose
/// strip is `strip`, first in the day after the day break that follows it; with no day
/// break after it, one is added right after the strip and the part goes below it.
/// Returns the new part's strip.
pub fn continue_scene(s: &mut Schedule, board: u32, strip: u32) -> EditResult<u32> {
    let board = board_id(s, board)?;
    let b = s.board(board).expect("board_id checked it");
    let found = b.sub_boards.iter().find_map(|sb| {
        sb.strips
            .iter()
            .position(|st| st.id == StripId(strip))
            .map(|i| (sb, i))
    });
    let Some((sb, i)) = found else {
        return Err(if kind_of(s, board, StripId(strip)).is_some() {
            "Schedule the scene on a sub-board before it spans days.".to_string()
        } else {
            "That strip is no longer on the board.".to_string()
        });
    };
    let StripKind::Sheet(sheet) = sb.strips[i].kind else {
        return Err("Only a scene can span days.".to_string());
    };
    let sub = sb.id;
    let next_break = sb.strips[i + 1..]
        .iter()
        .position(|st| st.kind == StripKind::DayBreak)
        .map(|k| i + 1 + k);
    let at = match next_break {
        Some(j) => j + 1,
        None => {
            s.insert_strip(
                board,
                StripKind::DayBreak,
                Place {
                    sub_board: Some(sub),
                    index: i + 1,
                },
            )
            .map_err(say)?;
            i + 2
        }
    };
    s.add_part(
        board,
        sheet,
        Place {
            sub_board: Some(sub),
            index: at,
        },
    )
    .map(|id| id.0)
    .map_err(say)
}

/// Replaces a sub-board's day breaks with new ones every `pages` (typed like `5` or
/// `4 4/8`): a day closes as soon as it reaches or passes the limit (decisions.md, 12).
pub fn auto_day_breaks(s: &mut Schedule, sub: u32, pages: &str) -> EditResult {
    let (board, sub) = sub_board(s, sub)?;
    let limit: Eighths = pages.parse().map_err(|e| format!("Pages per day: {e}."))?;
    if limit == Eighths::ZERO {
        return Err("Pages per day must be more than 0.".to_string());
    }
    engine_auto_breaks(s, board, sub, Limit::Pages(limit)).map_err(say)
}

/// Replaces a sub-board's day breaks with new ones, limiting each day `by` `pages` (typed
/// like `5` or `4 4/8`), shoot `time` (`h:mm`, banners count) or `screen_time` (`m:ss`):
/// a day closes as soon as it reaches or passes the limit (decisions.md, entries 12 and
/// 56). Returns how many shoot days that makes.
pub fn auto_day_breaks_by(s: &mut Schedule, sub: u32, by: &str, limit: &str) -> EditResult<usize> {
    let limit = match by {
        "pages" => return auto_day_breaks(s, sub, limit).and_then(|()| day_count(s, sub)),
        "time" => match crate::sheet::parse_duration(limit) {
            Ok(Some(m)) if m.0 > 0 => Limit::Time(m),
            Ok(_) => return Err("Shoot time per day must be more than 0:00.".to_string()),
            Err(why) => return Err(duration_say("Shoot time per day", limit, why)),
        },
        "screen_time" => {
            let t = crate::sheet::parse_screen_time(limit)
                .map_err(|e| e.replacen("Screen time", "Screen time per day", 1))?;
            match t {
                Some(x) if x.0 > 0 => Limit::ScreenTime(x),
                _ => return Err("Screen time per day must be more than 0:00.".to_string()),
            }
        }
        _ => return Err(format!("{by:?} is not a way to break days.")),
    };
    let (board, sub_id) = sub_board(s, sub)?;
    engine_auto_breaks(s, board, sub_id, limit).map_err(say)?;
    day_count(s, sub)
}

fn day_count(s: &Schedule, sub: u32) -> EditResult<usize> {
    let (board, sub) = sub_board(s, sub)?;
    method_schedule_core::days::days(s, board, sub)
        .map(|d| d.days.len())
        .map_err(say)
}

/// Removes every day break from a sub-board (Remove All Day Breaks): its strips stay in
/// order, unscheduled. Returns how many went; none is refused, so nothing is undone for
/// nothing.
pub fn remove_day_breaks(s: &mut Schedule, sub: u32) -> EditResult<usize> {
    let (board, sub) = sub_board(s, sub)?;
    match engine_remove_breaks(s, board, sub).map_err(say)? {
        0 => Err("There are no day breaks to remove.".to_string()),
        n => Ok(n),
    }
}

/// Sets a sub-board's start date (as typed, `2027-03-01` or `3/1/2027`; see
/// `calendar::typed_date`), or clears it with `None` or an empty string.
pub fn set_start_date(s: &mut Schedule, sub: u32, date: Option<&str>) -> EditResult {
    let (board, sub) = sub_board(s, sub)?;
    let start = match date.map(str::trim).filter(|d| !d.is_empty()) {
        None => None,
        Some(d) => Some(crate::calendar::parse_date("The start date", d)?),
    };
    s.set_start(board, sub, start).map_err(say)
}

// ---------------------------------------------------------------------------
// Banners

/// A banner's look as the page sends it (decisions.md, entries 65 and 87): bold, a palette
/// color (`BannerColor::text`: a name or `#rrggbb`; null or empty for the board's), two
/// lines high, a text size step (`small`, `normal`, `large`; empty for normal) and where
/// the text sits (`left`, `center`, `right`; empty for the left), and its font family
/// (`serif`, `sans`, `mono`; empty or `usual` for the board's; decision 187).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerStyleInput {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub tall: bool,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub align: String,
    #[serde(default)]
    pub font: String,
}

/// A text size step by name; empty is the usual size.
fn text_size(name: &str) -> EditResult<TextSize> {
    if name.is_empty() {
        return Ok(TextSize::Normal);
    }
    TextSize::from_name(name).ok_or_else(|| format!("{name:?} is not a text size."))
}

/// An alignment by name; empty is the usual way.
fn align(name: &str) -> EditResult<Align> {
    if name.is_empty() {
        return Ok(Align::Auto);
    }
    Align::from_name(name).ok_or_else(|| format!("{name:?} is not an alignment."))
}

fn banner_style(input: &BannerStyleInput) -> EditResult<BannerStyle> {
    let color = match input
        .color
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        None => None,
        Some(c) => Some(
            BannerColor::from_text(c)
                .ok_or_else(|| format!("{c:?} is not one of the banner colors."))?,
        ),
    };
    // A banner's text sits to the left unless told otherwise: Left and Auto are one.
    let align = match align(&input.align)? {
        Align::Left => Align::Auto,
        a => a,
    };
    Ok(BannerStyle {
        bold: input.bold,
        color,
        tall: input.tall,
        size: text_size(&input.size)?,
        align,
        font: match input.font.as_str() {
            "" => BannerFont::Usual,
            f => BannerFont::from_name(f)
                .ok_or_else(|| format!("{f:?} is not one of the banner fonts."))?,
        },
    })
}

/// A banner from what the page typed: text (trimmed, not empty, at most [`MAX_BANNER`]
/// characters) and a duration as `h:mm` (empty for none), at most 24:00. The duration
/// counts toward the day's time. Plain style.
pub fn banner(text: &str, duration: &str) -> EditResult<Banner> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Type the banner's text.".to_string());
    }
    if text.chars().count() > MAX_BANNER {
        return Err(format!("A banner is at most {MAX_BANNER} characters."));
    }
    let duration = parse_duration(duration)
        .map_err(|why| duration_say("Duration", duration, why))?
        .unwrap_or_default();
    Ok(Banner {
        text: text.to_string(),
        duration,
        style: BannerStyle::default(),
    })
}

/// Inserts a banner at `index` in a sub-board. Returns the new strip's id.
pub fn insert_banner(
    s: &mut Schedule,
    sub: u32,
    index: usize,
    text: &str,
    duration: &str,
) -> EditResult<u32> {
    insert_banner_styled(s, sub, index, text, duration, None)
}

/// Inserts a banner with a style (none: plain). Returns the new strip's id.
pub fn insert_banner_styled(
    s: &mut Schedule,
    sub: u32,
    index: usize,
    text: &str,
    duration: &str,
    style: Option<&BannerStyleInput>,
) -> EditResult<u32> {
    let mut b = banner(text, duration)?;
    if let Some(st) = style {
        b.style = banner_style(st)?;
    }
    let (board, sb) = sub_board(s, sub)?;
    let at = Place {
        sub_board: Some(sb),
        index,
    };
    s.insert_strip(board, StripKind::Banner(b), at)
        .map(|id| id.0)
        .map_err(say)
}

/// Changes a banner's text and duration, keeping its style.
pub fn set_banner(
    s: &mut Schedule,
    board: u32,
    strip: u32,
    text: &str,
    duration: &str,
) -> EditResult {
    set_banner_styled(s, board, strip, text, duration, None)
}

/// Changes a banner's text, duration and (when given) style.
pub fn set_banner_styled(
    s: &mut Schedule,
    board: u32,
    strip: u32,
    text: &str,
    duration: &str,
    style: Option<&BannerStyleInput>,
) -> EditResult {
    let mut b = banner(text, duration)?;
    let board = board_id(s, board)?;
    match kind_of(s, board, StripId(strip)) {
        Some(StripKind::Banner(old)) => {
            b.style = match style {
                Some(st) => banner_style(st)?,
                None => old.style,
            };
            s.set_banner(board, StripId(strip), b).map_err(say)
        }
        Some(_) => Err("That strip is not a banner.".to_string()),
        None => Err("That banner is no longer on the board.".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Sorting

/// One sort key as the page sends it: a field name ([`SortField::name`]) and the way.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortKeyInput {
    pub field: String,
    #[serde(default)]
    pub descending: bool,
}

/// Sorts a sub-board's strips (or `board`'s Boneyard, `sub` `None`) by up to
/// [`MAX_SORT_KEYS`] header fields: banners go to the top, day breaks are removed, and
/// scenes follow in order (`Schedule::sort_strips`).
pub fn sort_strips(
    s: &mut Schedule,
    board: u32,
    sub: Option<u32>,
    keys: &[SortKeyInput],
) -> EditResult {
    let board = board_id(s, board)?;
    let place = place_on(s, board, sub, 0)?;
    if keys.is_empty() {
        return Err("Choose a field to sort by.".to_string());
    }
    if keys.len() > MAX_SORT_KEYS {
        return Err(format!("Sort by at most {MAX_SORT_KEYS} fields."));
    }
    let mut out = Vec::new();
    for k in keys {
        let field = SortField::from_name(&k.field)
            .ok_or_else(|| format!("{:?} is not a field strips can be sorted by.", k.field))?;
        if out.iter().any(|x: &SortKey| x.field == field) {
            return Err("Sort by each field once.".to_string());
        }
        out.push(SortKey {
            field,
            descending: k.descending,
        });
    }
    s.sort_strips(board, place.sub_board, &out).map_err(say)
}

// ---------------------------------------------------------------------------
// The strip layout (decisions.md, entries 39, 78 and 87)

/// One field as the page sends it: a field name (`StripField::name`), a width
/// (`StripWidth::name`), its line (1 to 4), how many of the first line's columns it covers
/// (lines 2 to 4), its alignment (`auto`, `left`, `center`, `right`; empty for auto), bold,
/// italic, a custom text field's text, and its line styles.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutColumnInput {
    pub field: String,
    pub width: String,
    #[serde(default = "one")]
    pub line: u8,
    #[serde(default = "one")]
    pub span: u8,
    #[serde(default)]
    pub align: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub text: String,
    /// The category an element count counts (decision 147); none: every element. The
    /// category a category's elements lists (decision 194).
    #[serde(default)]
    pub category: Option<u32>,
    /// A category's elements by board ID rather than name (decision 194).
    #[serde(default)]
    pub board_ids: bool,
    /// Line styles (decision 185): a line after the cell, a rule under it.
    #[serde(default)]
    pub rule_after: bool,
    #[serde(default)]
    pub rule_under: bool,
}

/// The strip designer's switches for the marks on strips (decision 136): `true` hides.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarksInput {
    #[serde(default)]
    pub hide_red_flags: bool,
    #[serde(default)]
    pub hide_conflicts: bool,
}

fn one() -> u8 {
    1
}

/// "the strip layout shows Set twice" -> "The strip layout shows Set twice."
fn sentence(why: &str) -> String {
    let mut out = why.to_string();
    if let Some(c) = out.get(..1) {
        out = c.to_uppercase() + &out[1..];
    }
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// A strip layout from what the page sent, at text size `size` (`small`, `normal`,
/// `large`; empty for normal): every field and width one the engine knows, and the
/// engine's rules (`StripLayout::problem`) said as a sentence.
pub fn strip_layout(columns: &[LayoutColumnInput], size: &str) -> EditResult<StripLayout> {
    strip_layout_with(columns, size, MarksInput::default())
}

/// [`strip_layout`] with the designer's mark switches.
pub fn strip_layout_with(
    columns: &[LayoutColumnInput],
    size: &str,
    marks: MarksInput,
) -> EditResult<StripLayout> {
    if columns.is_empty() {
        return Err("Choose at least one field for the strips to show.".to_string());
    }
    let mut out: Vec<StripColumn> = Vec::with_capacity(columns.len());
    for c in columns {
        let field = StripField::from_name(&c.field)
            .ok_or_else(|| format!("{:?} is not a field strips can show.", c.field))?;
        let width = StripWidth::from_name(&c.width)
            .ok_or_else(|| format!("{:?} is not a column width.", c.width))?;
        if !(1..=MAX_STRIP_LINES).contains(&c.line) {
            return Err(format!("A strip has lines 1 to {MAX_STRIP_LINES}."));
        }
        let text = c.text.trim();
        if text.chars().count() > MAX_COLUMN_TEXT {
            return Err(format!(
                "A strip's custom text is at most {MAX_COLUMN_TEXT} characters."
            ));
        }
        out.push(StripColumn {
            field,
            width,
            line: c.line - 1,
            // The first line's fields are the columns: one each.
            span: if c.line == 1 { 1 } else { c.span },
            align: align(&c.align)?,
            bold: c.bold,
            italic: c.italic,
            text: if field == StripField::Text {
                text.to_string()
            } else {
                String::new()
            },
            category: c.category.map(method_schedule_core::model::CategoryId),
            board_ids: c.board_ids,
            rule_after: c.rule_after,
            rule_under: c.rule_under,
        });
    }
    let layout = StripLayout {
        size: text_size(size)?,
        hide_red_flags: marks.hide_red_flags,
        hide_conflicts: marks.hide_conflicts,
        ..StripLayout::of(out)
    };
    match layout.problem() {
        Some(why) => Err(sentence(&why)),
        None => Ok(layout),
    }
}

/// Sets what `board`'s own scene strips show (the layout it uses until it picks a named
/// one, decisions.md, entries 39 and 78).
pub fn set_strip_layout(
    s: &mut Schedule,
    board: u32,
    columns: &[LayoutColumnInput],
    size: &str,
) -> EditResult {
    set_strip_layout_with(s, board, columns, size, MarksInput::default())
}

/// [`set_strip_layout`] with the designer's mark switches (decision 136).
pub fn set_strip_layout_with(
    s: &mut Schedule,
    board: u32,
    columns: &[LayoutColumnInput],
    size: &str,
    marks: MarksInput,
) -> EditResult {
    let board = board_id(s, board)?;
    let layout = strip_layout_with(columns, size, marks)?;
    s.set_strip_layout(board, layout).map_err(say)
}

// ---------------------------------------------------------------------------
// What day breaks show (decisions.md, entries 67 and 87)

/// A board's day look as the page sends it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DayLookInput {
    #[serde(default)]
    pub shoot_day_strip: bool,
    #[serde(default)]
    pub screen_time: bool,
    #[serde(default)]
    pub sunrise: bool,
    #[serde(default)]
    pub sunset: bool,
    /// The day's element quantities (decision 87).
    #[serde(default)]
    pub quantity: bool,
    #[serde(default)]
    pub text: String,
}

/// A day look from what the page sent: the switches, and a line of text (trimmed, one
/// line, at most `MAX_DAY_TEXT` characters). Shared by a board's own day look and the
/// named day break and shoot day layouts.
pub fn day_look(input: &DayLookInput) -> EditResult<method_schedule_core::model::DayLook> {
    use method_schedule_core::model::{DayLook, MAX_DAY_TEXT};
    let text = input.text.trim();
    if text.chars().count() > MAX_DAY_TEXT {
        return Err(format!(
            "A day break's text is at most {MAX_DAY_TEXT} characters."
        ));
    }
    if text.contains(['\n', '\r']) {
        return Err("A day break's text is one line.".to_string());
    }
    Ok(DayLook {
        shoot_day_strip: input.shoot_day_strip,
        screen_time: input.screen_time,
        sunrise: input.sunrise,
        sunset: input.sunset,
        quantity: input.quantity,
        text: text.to_string(),
    })
}

/// Sets what `board`'s day breaks show: a shoot day strip heading each day, the day's screen
/// time, sunrise and sunset, its element quantities, and a line of text.
pub fn set_day_look(s: &mut Schedule, board: u32, input: &DayLookInput) -> EditResult {
    let board = board_id(s, board)?;
    let look = day_look(input)?;
    s.set_day_look(board, look).map_err(say)
}

// ---------------------------------------------------------------------------
// Strip colors (decisions.md, entry 60)

/// One strip color as the page sends it: I/E and time of day as typed (empty matches
/// any) and the color as `#rrggbb`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorRuleInput {
    pub int_ext: String,
    pub time_of_day: String,
    pub color: String,
    /// The pair's own text color, `#rrggbb`; none or empty follows the scene text color.
    #[serde(default)]
    pub text: Option<String>,
}

fn color(what: &str, text: &str) -> EditResult<method_schedule_core::model::Rgb> {
    method_schedule_core::model::Rgb::parse(text.trim())
        .ok_or_else(|| format!("{what} {:?} is not a color like #1f3524.", text.trim()))
}

/// Sets the schedule's strip colors: the rules in order (the first that matches a scene
/// wins), and a text color per strip type (`None` or empty for automatic).
pub fn set_strip_colors(
    s: &mut Schedule,
    rules: &[ColorRuleInput],
    scene_text: Option<&str>,
    banner_text: Option<&str>,
    break_text: Option<&str>,
) -> EditResult {
    use method_schedule_core::model::{StripColorRule, StripColors, MAX_COLOR_RULES};
    if rules.len() > MAX_COLOR_RULES {
        return Err(format!("Set at most {MAX_COLOR_RULES} strip colors."));
    }
    let mut out = Vec::with_capacity(rules.len());
    for r in rules {
        let (ie, tod) = (r.int_ext.trim(), r.time_of_day.trim());
        if ie.chars().count() > MAX_NAME || tod.chars().count() > MAX_NAME {
            return Err(format!(
                "A strip color's I/E or time of day is at most {MAX_NAME} characters."
            ));
        }
        let rule = StripColorRule {
            int_ext: ie.to_string(),
            time_of_day: tod.to_string(),
            color: color("Strip color", &r.color)?,
            text: match r.text.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
                None => None,
                Some(x) => Some(color("Text color", x)?),
            },
        };
        if out.iter().any(|o: &StripColorRule| {
            o.int_ext.eq_ignore_ascii_case(ie) && o.time_of_day.eq_ignore_ascii_case(tod)
        }) {
            return Err(format!("{} has two strip colors.", rule.label()));
        }
        out.push(rule);
    }
    let text = |what: &str, v: Option<&str>| match v.map(str::trim).filter(|x| !x.is_empty()) {
        None => Ok(None),
        Some(x) => color(what, x).map(Some),
    };
    let colors = StripColors {
        rules: out,
        scene_text: text("Scene text color", scene_text)?,
        banner_text: text("Banner text color", banner_text)?,
        break_text: text("Day break text color", break_text)?,
    };
    s.set_strip_colors(colors).map_err(say)
}

// ---------------------------------------------------------------------------
// Boards and sub-boards

fn name_text(value: &str) -> EditResult<String> {
    let t = value.trim();
    if t.is_empty() {
        return Err("Type a name.".to_string());
    }
    if t.chars().count() > MAX_NAME {
        return Err(format!("A name is at most {MAX_NAME} characters."));
    }
    Ok(t.to_string())
}

/// Adds a board with one sub-board, Main, starting `start` (the app passes today, as the
/// reference app does: behavior note 27, decision 153), and every scene in its Boneyard.
/// Returns its id.
pub fn add_board(
    s: &mut Schedule,
    name: &str,
    start: Option<method_schedule_core::Date>,
) -> EditResult<u32> {
    let name = s.board_name(&name_text(name)?, None).map_err(say)?;
    Ok(s.add_board(&name, start).0)
}

/// Copies a board (its sub-boards, calendars, strip order, banners and day breaks) under
/// a new name. Returns the copy's id.
pub fn duplicate_board(s: &mut Schedule, board: u32, name: &str) -> EditResult<u32> {
    let board = board_id(s, board)?;
    let name = s.board_name(&name_text(name)?, None).map_err(say)?;
    s.duplicate_board(board, &name).map(|b| b.0).map_err(say)
}

pub fn rename_board(s: &mut Schedule, board: u32, name: &str) -> EditResult {
    let board = board_id(s, board)?;
    s.rename_board(board, &name_text(name)?).map_err(say)
}

/// Moves a board to place `to` among the boards.
pub fn move_board(s: &mut Schedule, board: u32, to: usize) -> EditResult {
    let board = board_id(s, board)?;
    s.move_board(board, to).map_err(say)
}

/// Deletes a board (the page asks first). The scenes stay on every other board.
pub fn delete_board(s: &mut Schedule, board: u32) -> EditResult {
    let board = board_id(s, board)?;
    if s.boards.len() == 1 {
        return Err("A schedule keeps at least one board.".to_string());
    }
    s.delete_board(board).map_err(say)
}

/// Adds a sub-board at the end of `board`, starting `start` (the app passes today, as
/// the reference app does: behavior note 24, decision 123) and with the default week.
/// Returns its id.
pub fn add_sub_board(
    s: &mut Schedule,
    board: u32,
    name: &str,
    start: Option<method_schedule_core::Date>,
) -> EditResult<u32> {
    let board = board_id(s, board)?;
    let name = s
        .sub_board_name(board, &name_text(name)?, None)
        .map_err(say)?;
    s.add_sub_board(board, &name, start)
        .map(|x| x.0)
        .map_err(say)
}

pub fn rename_sub_board(s: &mut Schedule, sub: u32, name: &str) -> EditResult {
    let (board, sub) = sub_board(s, sub)?;
    s.rename_sub_board(board, sub, &name_text(name)?)
        .map_err(say)
}

/// Moves a sub-board to place `to` on its board.
pub fn move_sub_board(s: &mut Schedule, sub: u32, to: usize) -> EditResult {
    let (board, sub) = sub_board(s, sub)?;
    s.move_sub_board(board, sub, to).map_err(say)
}

/// Deletes a sub-board (the page asks first): its scenes go to the end of the Boneyard,
/// its day breaks, banners and calendar go with it.
pub fn delete_sub_board(s: &mut Schedule, sub: u32) -> EditResult {
    let (board, sub) = sub_board(s, sub)?;
    if s.board(board).is_some_and(|b| b.sub_boards.len() == 1) {
        return Err("A board keeps at least one sub-board.".to_string());
    }
    s.delete_sub_board(board, sub).map_err(say)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::Sheet;

    /// A schedule with scenes 1 to 4 on Main and 5 in the Boneyard.
    fn sample() -> (Session, u32, u32) {
        let mut s = Session::new();
        s.edit(|x| {
            for n in 1..=5 {
                x.add_sheet(Sheet {
                    scenes: n.to_string(),
                    pages: Eighths(8),
                    ..Sheet::default()
                });
            }
            let b = x.boards[0].id;
            let sb = x.boards[0].sub_boards[0].id;
            let ids: Vec<StripId> = x.boards[0].boneyard[..4].iter().map(|st| st.id).collect();
            x.move_strips(
                b,
                &ids,
                Place {
                    sub_board: Some(sb),
                    index: 0,
                },
            )
            .map_err(say)
        })
        .unwrap();
        let b = s.schedule().boards[0].id.0;
        let sb = s.schedule().boards[0].sub_boards[0].id.0;
        (s, b, sb)
    }

    fn scenes(s: &Schedule, sub: Option<u32>) -> Vec<String> {
        let b = &s.boards[0];
        let list = match sub {
            None => &b.boneyard,
            Some(id) => &b.sub_boards.iter().find(|x| x.id.0 == id).unwrap().strips,
        };
        list.iter()
            .map(|st| match &st.kind {
                StripKind::Sheet(id) => s.sheet(*id).unwrap().scenes.clone(),
                StripKind::Banner(b) => format!("[{}]", b.text),
                StripKind::DayBreak => "|".to_string(),
            })
            .collect()
    }

    fn strip_ids(s: &Schedule, sub: u32) -> Vec<u32> {
        s.boards[0]
            .sub_boards
            .iter()
            .find(|x| x.id.0 == sub)
            .unwrap()
            .strips
            .iter()
            .map(|st| st.id.0)
            .collect()
    }

    #[test]
    fn a_dragged_selection_moves_as_one_undo_step() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        let ids = strip_ids(s.schedule(), sb);
        // Scenes 1 and 2 (dropped below scene 4), across the day break.
        s.edit(|x| move_strips(x, b, &[ids[0], ids[1]], Some(sb), 5))
            .unwrap();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["|", "3", "4", "1", "2"]);
        // Into the Boneyard, at its top.
        s.edit(|x| move_strips(x, b, &[ids[3], ids[4]], None, 0))
            .unwrap();
        assert_eq!(scenes(s.schedule(), None), ["3", "4", "5"]);
        s.undo();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["|", "3", "4", "1", "2"]);
        s.undo();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["1", "2", "|", "3", "4"]);
    }

    #[test]
    fn day_breaks_park_in_the_boneyard_but_banners_never_go_there() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 1)).unwrap();
        s.edit(|x| insert_day_break(x, sb, 3)).unwrap();
        let banner = s.edit(|x| insert_banner(x, sb, 0, "Move", "0:30")).unwrap();
        let ids = strip_ids(s.schedule(), sb);
        let before = s.schedule().clone();
        let e = s
            .edit(|x| move_strips(x, b, &[banner, ids[2]], None, 0))
            .unwrap_err();
        assert_eq!(e, "Banners go on a sub-board, not in the Boneyard.");
        assert_eq!(s.schedule(), &before);
        // Scene 1's day break parks in the Boneyard: no day, no date, nothing counted.
        s.edit(|x| move_strips(x, b, &[ids[2]], None, 0)).unwrap();
        assert_eq!(scenes(s.schedule(), None), ["|", "5"]);
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].days.len(), 1);
        assert_eq!(v.board.boneyard[0].kind, "break");
        s.schedule().check().unwrap();
        // And comes back.
        s.edit(|x| move_strips(x, b, &[ids[2]], Some(sb), 2))
            .unwrap();
        assert_eq!(s.schedule().boards[0].boneyard.len(), 1);
        s.edit(|x| move_strips(x, b, &[ids[2]], None, 0)).unwrap();
        // A sub-board of another board is refused.
        let other = s.edit(|x| add_board(x, "Rain plan", None)).unwrap();
        let other_sub = s.schedule().board(BoardId(other)).unwrap().sub_boards[0]
            .id
            .0;
        assert!(s
            .edit(|x| move_strips(x, b, &[ids[0]], Some(other_sub), 0))
            .is_err());
    }

    #[test]
    fn banners_are_inserted_edited_and_deleted() {
        let (mut s, b, sb) = sample();
        let id = s
            .edit(|x| insert_banner(x, sb, 1, " Company move ", "0:45"))
            .unwrap();
        assert_eq!(scenes(s.schedule(), Some(sb))[1], "[Company move]");
        let days = |s: &Session| {
            crate::view::view(s, None).unwrap().board.sub_boards[0]
                .days
                .clone()
        };
        s.edit(|x| insert_day_break(x, sb, 6)).unwrap();
        assert_eq!(days(&s)[0].time, "0:45");
        s.edit(|x| set_banner(x, b, id, "Lunch", "1:00")).unwrap();
        assert_eq!(days(&s)[0].time, "1:00");
        assert!(s.edit(|x| set_banner(x, b, id, "  ", "1:00")).is_err());
        assert!(s.edit(|x| set_banner(x, b, id, "Lunch", "90")).is_err());
        // A duration is a day at most (decision 50): 900:45 was taken before.
        s.edit(|x| set_banner(x, b, id, "Lunch", "24:00")).unwrap();
        assert_eq!(days(&s)[0].time, "24:00");
        let before = s.schedule().clone();
        assert_eq!(
            s.edit(|x| set_banner(x, b, id, "Lunch", " 900:45 "))
                .unwrap_err(),
            "Duration \"900:45\" is longer than a day. Type 24:00 at most."
        );
        assert!(s
            .edit(|x| insert_banner(x, sb, 0, "Move", "24:01"))
            .is_err());
        assert_eq!(s.schedule(), &before);
        assert!(s
            .edit(|x| insert_banner(x, sb, 0, &"x".repeat(201), ""))
            .is_err());
        let scene = strip_ids(s.schedule(), sb)[0];
        assert!(s.edit(|x| set_banner(x, b, scene, "Lunch", "")).is_err());
        let brk = *strip_ids(s.schedule(), sb).last().unwrap();
        s.edit(|x| delete_strips(x, b, &[id, brk])).unwrap();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["1", "2", "3", "4"]);
    }

    #[test]
    fn deleting_a_scene_strip_deletes_its_sheet_on_every_board() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        let copy = s.edit(|x| duplicate_board(x, b, "Rain plan")).unwrap();
        let ids = strip_ids(s.schedule(), sb);
        let before = s.schedule().clone();
        // Scene 1, the day break and scene 4, as one step.
        s.edit(|x| delete_strips(x, b, &[ids[0], ids[2], ids[4]]))
            .unwrap();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["2", "3"]);
        assert_eq!(s.schedule().sheets.len(), 3);
        let other = s.schedule().board(BoardId(copy)).unwrap();
        assert_eq!(
            other.sub_boards[0].strips.len(),
            3,
            "2, its own day break, 3"
        );
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");
        // A Boneyard scene too; an unknown strip refuses the whole edit.
        let yard = s.schedule().boards[0].boneyard[0].id.0;
        assert!(s.edit(|x| delete_strips(x, b, &[yard, 999_999])).is_err());
        assert_eq!(s.schedule(), &before);
        s.edit(|x| delete_strips(x, b, &[yard])).unwrap();
        assert!(s.schedule().boards[0].boneyard.is_empty());
    }

    #[test]
    fn days_break_by_pages_shoot_time_or_screen_time_and_all_breaks_go_at_once() {
        let (mut s, _, sb) = sample();
        s.edit(|x| {
            for (i, h) in x.sheets.iter_mut().enumerate() {
                h.shoot_time = Some(method_schedule_core::model::Minutes(60 * (i as u32 + 1)));
                h.screen_time = Some(method_schedule_core::model::Seconds(30));
            }
            Ok(())
        })
        .unwrap();
        // Shoot time 1:00, 2:00, 3:00, 4:00 with 3:00 a day: [1 + 2], [3], [4].
        assert_eq!(
            s.edit(|x| auto_day_breaks_by(x, sb, "time", "3:00"))
                .unwrap(),
            3
        );
        assert_eq!(
            scenes(s.schedule(), Some(sb)),
            ["1", "2", "|", "3", "|", "4", "|"]
        );
        // Screen time 0:30 each with 1:00 a day: two scenes a day.
        assert_eq!(
            s.edit(|x| auto_day_breaks_by(x, sb, "screen_time", "1:00"))
                .unwrap(),
            2
        );
        assert_eq!(
            s.edit(|x| auto_day_breaks_by(x, sb, "pages", "2")).unwrap(),
            2
        );
        let before = s.schedule().clone();
        for (by, bad) in [
            ("time", "0:00"),
            ("time", "90"),
            ("time", "25:00"),
            ("screen_time", ""),
            ("screen_time", "1.5"),
            ("pages", "0"),
            ("hours", "1"),
        ] {
            let e = s.edit(|x| auto_day_breaks_by(x, sb, by, bad)).unwrap_err();
            assert!(e.ends_with('.'), "{e}");
        }
        assert_eq!(s.schedule(), &before);
        // Remove all: one step, the strips stay in order.
        assert_eq!(s.edit(|x| remove_day_breaks(x, sb)).unwrap(), 2);
        assert_eq!(scenes(s.schedule(), Some(sb)), ["1", "2", "3", "4"]);
        assert_eq!(
            s.edit(|x| remove_day_breaks(x, sb)).unwrap_err(),
            "There are no day breaks to remove."
        );
        assert!(s.undo());
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn strip_colors_are_set_as_one_step_and_the_board_draws_them() {
        let (mut s, _, _) = sample();
        s.edit(|x| {
            let id = x.sheets[0].id;
            let h = x.sheet_mut(id).unwrap();
            h.int_ext = "EXT".into();
            h.time_of_day = "NIGHT".into();
            Ok(())
        })
        .unwrap();
        let rule = |ie: &str, tod: &str, c: &str| ColorRuleInput {
            int_ext: ie.into(),
            time_of_day: tod.into(),
            color: c.into(),
            text: None,
        };
        let before = s.schedule().clone();
        s.edit(|x| {
            set_strip_colors(
                x,
                &[rule(" ext ", "Night", "#1F3524")],
                None,
                Some("#6a1b9a"),
                Some(""),
            )
        })
        .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let first = &v.board.sub_boards[0].unscheduled[0];
        assert_eq!(
            (first.color.as_str(), first.ink.as_str()),
            ("#1f3524", "#ffffff")
        );
        assert!(
            v.board.sub_boards[0].unscheduled[1].color.is_empty(),
            "no rule: the usual tone"
        );
        assert_eq!(v.board.colors.rules[0].int_ext, "ext");
        assert_eq!(v.board.colors.banner_text.as_deref(), Some("#6a1b9a"));
        assert_eq!(v.board.colors.break_text, None);
        // A pair's own text color wins over the scene text color (decision 108).
        let mut inked = rule("EXT", "NIGHT", "#1f3524");
        inked.text = Some("#FFEEAA".into());
        s.edit(|x| set_strip_colors(x, &[inked.clone()], Some("#000000"), None, None))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].unscheduled[0].ink, "#ffeeaa");
        assert_eq!(v.board.colors.rules[0].text.as_deref(), Some("#ffeeaa"));
        inked.text = Some(" ".into());
        s.edit(|x| set_strip_colors(x, &[inked.clone()], Some("#000000"), None, None))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].unscheduled[0].ink, "#000000");
        inked.text = Some("pink".into());
        assert!(s
            .edit(|x| set_strip_colors(x, &[inked.clone()], None, None, None))
            .is_err());
        assert!(s.undo() && s.undo());
        assert!(s.undo());
        assert_eq!(s.schedule(), &before);
        for bad in [
            vec![rule("EXT", "NIGHT", "green")],
            vec![rule("EXT", "NIGHT", "#123")],
            vec![rule("EXT", "", "#000000"), rule("ext", " ", "#ffffff")],
        ] {
            let e = s
                .edit(|x| set_strip_colors(x, &bad, None, None, None))
                .unwrap_err();
            assert!(e.ends_with('.'), "{e}");
        }
        assert!(s
            .edit(|x| set_strip_colors(x, &[], Some("black"), None, None))
            .is_err());
        assert_eq!(s.schedule(), &before);
    }

    #[test]
    fn banners_take_a_style_that_editing_the_text_keeps() {
        let (mut s, b, sb) = sample();
        let style = BannerStyleInput {
            bold: true,
            color: Some("red".into()),
            tall: true,
            size: "large".into(),
            align: "center".into(),
            font: "serif".into(),
        };
        let id = s
            .edit(|x| insert_banner_styled(x, sb, 0, "Company move", "0:30", Some(&style)))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let dto = &v.board.sub_boards[0].unscheduled[0];
        assert_eq!(
            (dto.bold, dto.banner_color.as_str(), dto.tall),
            (true, "red", true)
        );
        // Its size and alignment (decisions.md, entry 87), and font family (entry 187).
        assert_eq!((dto.banner_size, dto.banner_align), ("large", "center"));
        assert_eq!(dto.banner_font, "serif");
        // Editing the text alone keeps the style; a style given replaces it.
        s.edit(|x| set_banner(x, b, id, "Lunch", "1:00")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert!(v.board.sub_boards[0].unscheduled[0].bold);
        s.edit(|x| {
            set_banner_styled(
                x,
                b,
                id,
                "Lunch",
                "1:00",
                Some(&BannerStyleInput::default()),
            )
        })
        .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let dto = &v.board.sub_boards[0].unscheduled[0];
        assert_eq!(
            (dto.bold, dto.banner_color.as_str(), dto.tall),
            (false, "", false)
        );
        assert_eq!(
            (dto.banner_size, dto.banner_align, dto.banner_font),
            ("", "", "")
        );
        // Left is the usual place: it stores as nothing.
        let left = BannerStyleInput {
            align: "left".into(),
            ..BannerStyleInput::default()
        };
        s.edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&left)))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].unscheduled[0].banner_align, "");
        // A color by value from the palette (decision 154); uppercase is refused.
        let teal = BannerStyleInput {
            color: Some("#3fb8a8".into()),
            ..BannerStyleInput::default()
        };
        s.edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&teal)))
            .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].unscheduled[0].banner_color, "#3fb8a8");
        let upper = BannerStyleInput {
            color: Some("#3FB8A8".into()),
            ..BannerStyleInput::default()
        };
        assert!(s
            .edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&upper)))
            .is_err());
        for bad in [("size", "huge"), ("align", "middle")] {
            let style = BannerStyleInput {
                size: if bad.0 == "size" {
                    bad.1.into()
                } else {
                    String::new()
                },
                align: if bad.0 == "align" {
                    bad.1.into()
                } else {
                    String::new()
                },
                ..BannerStyleInput::default()
            };
            assert!(s
                .edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&style)))
                .is_err());
        }
        let bad = BannerStyleInput {
            color: Some("chartreuse".into()),
            ..BannerStyleInput::default()
        };
        assert!(s
            .edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&bad)))
            .is_err());
        let bad = BannerStyleInput {
            font: "Comic Sans".into(),
            ..BannerStyleInput::default()
        };
        assert!(s
            .edit(|x| set_banner_styled(x, b, id, "Lunch", "1:00", Some(&bad)))
            .is_err());
    }

    #[test]
    fn the_day_look_is_one_step_and_days_carry_their_extras() {
        let (mut s, b, sb) = sample();
        s.edit(|x| {
            for (i, h) in x.sheets.iter_mut().enumerate() {
                h.sunrise = Some(360 + i as u32);
                h.sunset = Some(1140 + i as u32);
                h.screen_time = Some(method_schedule_core::model::Seconds(65));
            }
            Ok(())
        })
        .unwrap();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        s.edit(|x| set_start_date(x, sb, Some("2027-03-03")))
            .unwrap();
        let before = s.schedule().clone();
        let look = DayLookInput {
            shoot_day_strip: true,
            screen_time: true,
            sunrise: true,
            sunset: true,
            quantity: false,
            text: "  Wrap by 19:00 ".into(),
        };
        s.edit(|x| set_day_look(x, b, &look)).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.day_look.text, "Wrap by 19:00");
        assert!(v.board.day_look.shoot_day_strip);
        let d = &v.board.sub_boards[0].days[0];
        assert_eq!((d.sunrise.as_str(), d.sunset.as_str()), ("06:00", "19:01"));
        assert_eq!(d.screen_time, "2:10");
        assert_eq!(d.full_date.as_deref(), Some("Wednesday, March 3, 2027"));
        assert!(s.undo());
        assert_eq!(s.schedule(), &before);
        let bad = DayLookInput {
            text: "two\nlines".into(),
            ..DayLookInput::default()
        };
        assert!(s.edit(|x| set_day_look(x, b, &bad)).is_err());
        let long = DayLookInput {
            text: "x".repeat(201),
            ..DayLookInput::default()
        };
        assert!(s.edit(|x| set_day_look(x, b, &long)).is_err());
    }

    /// Note 27: a day break's sun came only from times typed on its sheets. A sheet with
    /// none whose location has degrees gives the worked-out times (decision 167).
    #[test]
    fn day_breaks_show_the_sun_at_a_located_sheet() {
        let (mut s, _, sb) = sample();
        s.edit(|x| {
            for h in x.sheets.iter_mut() {
                h.location = "Pier".into();
            }
            x.plan
                .set_location(method_schedule_core::plan::GeoLocation {
                    name: "Pier".into(),
                    latitude: 34_052_200,
                    longitude: -118_243_700,
                    utc_offset: -420,
                    daylight_saving: None,
                })
                .map_err(say)
        })
        .unwrap();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        let day = |s: &Session| {
            let v = crate::view::view(s, None).unwrap();
            let d = &v.board.sub_boards[0].days[0];
            (d.sunrise.clone(), d.sunset.clone())
        };
        // No date yet: no sun.
        assert_eq!(day(&s), (String::new(), String::new()));
        s.edit(|x| set_start_date(x, sb, Some("2024-06-20")))
            .unwrap();
        let (rise, set) = day(&s);
        assert!(rise.starts_with("05:4"), "{rise}");
        assert!(set.starts_with("20:0"), "{set}");
        // A typed sunrise wins; the sunset is still worked out.
        s.edit(|x| {
            x.sheets[0].sunrise = Some(300);
            Ok(())
        })
        .unwrap();
        assert_eq!(day(&s), ("05:00".to_string(), set));
    }

    #[test]
    fn a_scene_continues_on_the_next_day_and_its_part_deletes_alone() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        let ids = strip_ids(s.schedule(), sb);
        // Scene 2 (before the break) continues first in day 2.
        let part = s.edit(|x| continue_scene(x, b, ids[1])).unwrap();
        assert_eq!(
            scenes(s.schedule(), Some(sb)),
            ["1", "2", "|", "2", "3", "4"]
        );
        let v = crate::view::view(&s, None).unwrap();
        let day2 = &v.board.sub_boards[0].days.len();
        assert_eq!(*day2, 1);
        let first = &v.board.sub_boards[0].days[0].strips[1];
        assert_eq!((first.part, first.parts), (1, 2));
        let tail = &v.board.sub_boards[0].unscheduled[0];
        assert_eq!((tail.id, tail.part, tail.parts), (part, 2, 2));
        // Scene 4 (no break after it): a break goes in, then the part.
        s.edit(|x| continue_scene(x, b, ids[4])).unwrap();
        assert_eq!(
            scenes(s.schedule(), Some(sb)),
            ["1", "2", "|", "2", "3", "4", "|", "4"]
        );
        // A part can't go to the Boneyard; deleting one leaves the sheet.
        assert!(s.edit(|x| move_strips(x, b, &[part], None, 0)).is_err());
        let sheets = s.schedule().sheets.len();
        s.edit(|x| delete_strips(x, b, &[part])).unwrap();
        assert_eq!(s.schedule().sheets.len(), sheets);
        assert!(
            s.edit(|x| continue_scene(x, b, ids[2])).is_err(),
            "a day break"
        );
        let yard = s.schedule().boards[0].boneyard[0].id.0;
        assert!(
            s.edit(|x| continue_scene(x, b, yard)).is_err(),
            "the Boneyard"
        );
    }

    #[test]
    fn sorting_is_one_step_and_checked() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        s.edit(|x| insert_banner(x, sb, 4, "Lunch", "1:00"))
            .unwrap();
        let key = |f: &str, d| SortKeyInput {
            field: f.to_string(),
            descending: d,
        };
        s.edit(|x| sort_strips(x, b, Some(sb), &[key("scene", true)]))
            .unwrap();
        assert_eq!(
            scenes(s.schedule(), Some(sb)),
            ["[Lunch]", "4", "3", "2", "1"]
        );
        s.undo();
        assert_eq!(
            scenes(s.schedule(), Some(sb)),
            ["1", "2", "|", "3", "[Lunch]", "4"]
        );
        s.edit(|x| sort_strips(x, b, None, &[key("pages", false)]))
            .unwrap();
        for bad in [
            vec![],
            vec![key("cast", false)],
            vec![key("set", false), key("set", true)],
            vec![
                key("set", false),
                key("unit", false),
                key("episode", false),
                key("scene", false),
                key("pages", false),
            ],
        ] {
            assert!(s.edit(|x| sort_strips(x, b, Some(sb), &bad)).is_err());
        }
        // Four rules are allowed (decision 122).
        let four = [
            key("set", false),
            key("unit", false),
            key("episode", false),
            key("scene", false),
        ];
        s.edit(|x| sort_strips(x, b, Some(sb), &four)).unwrap();
    }

    #[test]
    fn boards_and_sub_boards_are_managed() {
        let (mut s, b, sb) = sample();
        let copy = s.edit(|x| duplicate_board(x, b, "Rain plan")).unwrap();
        assert_eq!(scenes(s.schedule(), Some(sb)), ["1", "2", "3", "4"]);
        assert!(s.edit(|x| add_board(x, "rain plan", None)).is_err());
        assert!(s.edit(|x| add_board(x, " ", None)).is_err());
        // A new board's Main starts on the date given, today from the app (decision 153),
        // with every scene in its Boneyard.
        let day = "2027-03-01".parse().ok();
        let fresh = s.edit(|x| add_board(x, "Fresh", day)).unwrap();
        let fb = s.schedule().board(BoardId(fresh)).unwrap();
        assert_eq!(fb.sub_boards.len(), 1);
        assert_eq!(fb.sub_boards[0].start, day);
        assert_eq!(fb.boneyard.len(), s.schedule().sheets.len());
        s.edit(|x| delete_board(x, fresh)).unwrap();
        s.edit(|x| rename_board(x, copy, "Wet")).unwrap();
        s.edit(|x| move_board(x, copy, 0)).unwrap();
        assert_eq!(s.schedule().boards[0].name, "Wet");
        s.edit(|x| delete_board(x, copy)).unwrap();
        assert_eq!(
            s.edit(|x| delete_board(x, b)).unwrap_err(),
            "A schedule keeps at least one board."
        );

        let start = "2027-03-01".parse().ok();
        let unit = s.edit(|x| add_sub_board(x, b, "Unit B", start)).unwrap();
        assert!(s.edit(|x| add_sub_board(x, b, "unit b", None)).is_err());
        // It starts on the date given, today from the app (decision 123).
        assert_eq!(
            s.schedule().boards[0]
                .sub_boards
                .iter()
                .find(|x| x.id.0 == unit)
                .unwrap()
                .start,
            start
        );
        s.edit(|x| rename_sub_board(x, unit, "Second unit"))
            .unwrap();
        s.edit(|x| move_sub_board(x, unit, 0)).unwrap();
        s.edit(|x| set_start_date(x, unit, Some("2027-03-08")))
            .unwrap();
        let ids = strip_ids(s.schedule(), sb);
        s.edit(|x| move_strips(x, b, &ids[..2], Some(unit), 0))
            .unwrap();
        s.edit(|x| insert_day_break(x, unit, 2)).unwrap();
        s.edit(|x| delete_sub_board(x, unit)).unwrap();
        assert_eq!(scenes(s.schedule(), None), ["5", "1", "2"]);
        assert_eq!(
            s.edit(|x| delete_sub_board(x, sb)).unwrap_err(),
            "A board keeps at least one sub-board."
        );
        assert!(s.edit(|x| rename_sub_board(x, 9999, "x")).is_err());
    }

    fn col(field: &str, width: &str) -> LayoutColumnInput {
        LayoutColumnInput {
            field: field.to_string(),
            width: width.to_string(),
            line: 1,
            span: 1,
            align: String::new(),
            bold: false,
            italic: false,
            text: String::new(),
            category: None,
            board_ids: false,
            rule_after: false,
            rule_under: false,
        }
    }

    /// Decision 194: a category's elements takes a category and the board ID switch from
    /// the page; the strips, its heading and the one-line report that follows the board's
    /// layout list them.
    #[test]
    fn the_strip_designer_takes_lines_spans_looks_and_text_from_the_page() {
        // Decisions.md, entry 87.
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        let on = |field: &str, line: u8, span: u8| LayoutColumnInput {
            line,
            span,
            ..col(field, "narrow")
        };
        let cols = [
            LayoutColumnInput {
                bold: true,
                align: "center".into(),
                ..on("scene", 1, 1)
            },
            LayoutColumnInput {
                rule_after: true,
                ..on("set", 1, 1)
            },
            on("pages", 1, 1),
            LayoutColumnInput {
                italic: true,
                rule_under: true,
                ..on("synopsis", 2, 2)
            },
            on("spacer", 2, 1),
            LayoutColumnInput {
                text: "  Call 07:00 ".into(),
                ..on("text", 4, 3)
            },
            on("spacer", 4, 0),
        ];
        // A span of 0 is refused by the engine's rule, said as a sentence.
        let e = s
            .edit(|x| set_strip_layout(x, b, &cols, "large"))
            .unwrap_err();
        assert!(e.starts_with('S') && e.ends_with('.'), "{e}");
        let cols = &cols[..6];
        s.edit(|x| set_strip_layout(x, b, cols, "large")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.layout_size, "large");
        let l = &v.board.layout;
        assert_eq!((l[0].align, l[0].bold), ("center", true));
        assert_eq!((l[3].line, l[3].span, l[3].italic), (2, 2, true));
        // Line styles (decision 185) reach the page.
        assert_eq!((l[1].rule_after, l[1].rule_under), (true, false));
        assert_eq!((l[3].rule_after, l[3].rule_under), (false, true));
        assert_eq!(
            (l[5].field, l[5].line, l[5].text.as_str()),
            ("text", 4, "Call 07:00")
        );
        // Every strip carries a cell per field; the text is the layout's, not each strip's.
        let strip = &v.board.sub_boards[0].days[0].strips[0];
        assert_eq!(strip.cells.len(), 6);
        assert_eq!(strip.cells[5], "");
        // Refused: a fifth line, a line wider than the first, an alignment or size unknown.
        for (bad, size) in [
            (vec![on("scene", 5, 1)], ""),
            (vec![on("scene", 1, 1), on("set", 2, 2)], ""),
            (
                vec![LayoutColumnInput {
                    align: "middle".into(),
                    ..on("scene", 1, 1)
                }],
                "",
            ),
            (vec![on("scene", 1, 1)], "huge"),
        ] {
            let e = s.edit(|x| set_strip_layout(x, b, &bad, size)).unwrap_err();
            assert!(e.ends_with('.'), "{e}");
        }
        // A span asked for on the first line is one column there.
        s.edit(|x| set_strip_layout(x, b, &[on("scene", 1, 3)], ""))
            .unwrap();
        assert_eq!(s.schedule().boards[0].strip_layout.columns[0].span, 1);
    }

    #[test]
    fn day_breaks_show_the_days_quantities_when_asked() {
        // Decisions.md, entry 87: the elements lane's daily quantities on the day break.
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 2)).unwrap();
        s.edit(|x| {
            let crowd = x.add_category("Background");
            x.category_mut(crowd).unwrap().daily_quantity = true;
            let dock = x.add_element(crowd, "Dock workers").unwrap();
            let (h0, h1) = (x.sheets[0].id, x.sheets[1].id);
            x.tag(h0, dock).unwrap();
            x.tag(h1, dock).unwrap();
            x.set_quantity(h0, dock, 20, true).unwrap();
            x.set_quantity(h1, dock, 5, true).unwrap();
            Ok(())
        })
        .unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.sub_boards[0].days[0].quantity, "", "off by default");
        let look = DayLookInput {
            quantity: true,
            ..DayLookInput::default()
        };
        s.edit(|x| set_day_look(x, b, &look)).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert!(v.board.day_look.quantity);
        assert_eq!(v.board.sub_boards[0].days[0].quantity, "Background 25");
    }

    /// The page's list of fields and widths (ui/js/strips.js) is the engine's, in order,
    /// with the same default widths.
    #[test]
    fn the_pages_layout_fields_are_the_engines() {
        use method_schedule_core::model::{StripField, StripWidth};
        const JS: &str = include_str!("../../method-schedule-app/ui/js/strips.js");
        let block = |start: &str| -> Vec<Vec<String>> {
            let from = JS.split(start).nth(1).expect("list in strips.js");
            let body = from.split("];").next().unwrap();
            body.lines()
                .filter_map(|l| l.trim().strip_prefix('['))
                .map(|l| {
                    l.split('\'')
                        .enumerate()
                        .filter(|(i, _)| i % 2 == 1)
                        .map(|(_, s)| s.to_string())
                        .collect()
                })
                .collect()
        };
        let fields = block("export const LAYOUT_FIELDS = [");
        let want: Vec<Vec<String>> = StripField::ALL
            .iter()
            .map(|f| {
                vec![
                    f.name().to_string(),
                    f.label().to_string(),
                    f.default_width().name().to_string(),
                ]
            })
            .collect();
        assert_eq!(fields, want);
        let widths: Vec<String> = block("export const LAYOUT_WIDTHS = [")
            .into_iter()
            .map(|w| w[0].clone())
            .collect();
        let want: Vec<String> = StripWidth::ALL
            .iter()
            .map(|w| w.name().to_string())
            .collect();
        assert_eq!(widths, want);
        // The designer's alignments, text sizes and line count (decisions.md, entry 87).
        let names = |start: &str| -> Vec<String> {
            block(start).into_iter().map(|w| w[0].clone()).collect()
        };
        let want: Vec<String> = Align::ALL.iter().map(|a| a.name().to_string()).collect();
        assert_eq!(names("const LAYOUT_ALIGNS = ["), want);
        let want: Vec<String> = TextSize::ALL.iter().map(|t| t.name().to_string()).collect();
        assert_eq!(names("export const TEXT_SIZES = ["), want);
        assert!(JS.contains(&format!("export const MAX_LINES = {MAX_STRIP_LINES};")));
    }

    /// The board's line height in the page (ui/js/board.js, LINE_PX) is the one the
    /// stylesheet gives a strip line, and each taller line a whole number of them, so the
    /// windowed board's arithmetic holds for strips of up to four lines (entry 87).
    #[test]
    fn the_boards_line_height_matches_its_stylesheet() {
        const JS: &str = include_str!("../../method-schedule-app/ui/js/board.js");
        const CSS: &str = include_str!("../../method-schedule-app/ui/styles.css");
        let px: u32 = JS
            .split("const LINE_PX = ")
            .nth(1)
            .and_then(|r| r.split(';').next())
            .and_then(|n| n.trim().parse().ok())
            .expect("LINE_PX in board.js");
        assert!(CSS.contains(&format!(".strips > .line {{ height: {px}px;")));
        for n in 2..=MAX_STRIP_LINES {
            let h = px * u32::from(n);
            assert!(
                CSS.contains(&format!(".strips > .line.h{n} {{ height: {h}px; }}")),
                "h{n}"
            );
        }
    }

    #[test]
    fn a_category_elements_field_lists_one_category() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        let props = s
            .edit(|x| {
                let props = x.add_category("Props");
                let rope = x.add_element(props, "ROPE").map_err(|e| e.to_string())?;
                let lamp = x.add_element(props, "LAMP").map_err(|e| e.to_string())?;
                x.set_board_id(lamp, Some("P1"))
                    .map_err(|e| e.to_string())?;
                let first = x.sheets[0].id;
                x.sheet_mut(first).unwrap().elements = vec![rope, lamp];
                Ok(props)
            })
            .unwrap();
        let list = |c: Option<u32>, ids: bool| LayoutColumnInput {
            category: c,
            board_ids: ids,
            ..col("category_elements", "wide")
        };
        let cols = [col("scene", "narrow"), list(Some(props.0), false)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.layout[1].label, "Props");
        let cells =
            |v: &crate::view::View, i: usize| v.board.sub_boards[0].days[0].strips[i].cells.clone();
        assert_eq!(cells(&v, 0), ["1", "LAMP, ROPE"]);
        assert_eq!(cells(&v, 1), ["2", ""]);
        let cols = [col("scene", "narrow"), list(Some(props.0), true)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert!(v.board.layout[1].board_ids);
        assert_eq!(cells(&v, 0), ["1", "P1, ROPE"]);
        // No category: an empty cell. The switch on another field is refused.
        let cols = [col("scene", "narrow"), list(None, true)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        assert_eq!(cells(&crate::view::view(&s, None).unwrap(), 0), ["1", ""]);
        let bad = [LayoutColumnInput {
            board_ids: true,
            ..col("scene", "narrow")
        }];
        let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
        assert!(e.ends_with('.'), "{e}");
    }

    #[test]
    fn the_strip_layout_is_one_step_and_the_board_and_one_line_follow_it() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        s.edit(|x| {
            let id = x.sheets[0].id;
            x.sheet_mut(id).unwrap().synopsis = "Tove mends the net.".into();
            Ok(())
        })
        .unwrap();
        let before = s.schedule().clone();
        let v = crate::view::view(&s, None).unwrap();
        let fields: Vec<&str> = v.board.layout.iter().map(|c| c.field).collect();
        assert_eq!(
            fields,
            ["scene", "int_ext", "set", "time_of_day", "pages", "cast"]
        );
        let cols = [
            col("synopsis", "widest"),
            col("scene", "narrow"),
            col("pages", "medium"),
        ];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        let labels: Vec<&str> = v.board.layout.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Synopsis", "Scene", "Pages"]);
        assert_eq!(v.board.layout[2].width, "medium");
        let first = &v.board.sub_boards[0].days[0].strips[0];
        assert_eq!(first.cells, ["Tove mends the net.", "1", "1"]);
        assert!(
            v.board.sub_boards[0].days[0].end.cells.is_empty(),
            "day breaks have none"
        );
        assert!(s.undo());
        assert_eq!(s.schedule(), &before, "one undo step");
        // Refused, nothing changed.
        for bad in [
            vec![],
            vec![col("scene", "narrow"), col("scene", "wide")],
            vec![col("colour", "narrow")],
            vec![col("scene", "huge")],
        ] {
            let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
            assert!(e.ends_with('.') && !e.contains('!'), "{e}");
        }
        assert!(s.edit(|x| set_strip_layout(x, 999_999, &cols, "")).is_err());
        assert_eq!(s.schedule(), &before);
    }

    /// BRD-18 (behavior note 27, decision 147): the element count takes one category from
    /// the page and the strips show `Name (n)`; the one-line report counts every element.
    #[test]
    fn an_element_count_takes_a_category_from_the_page() {
        let (mut s, b, sb) = sample();
        s.edit(|x| insert_day_break(x, sb, 4)).unwrap();
        let (cast, props) = s
            .edit(|x| {
                let cast = x.cast_or_add();
                let props = x.add_category("Props");
                let net = x.add_element(props, "Net").map_err(|e| e.to_string())?;
                let tove = x.add_element(cast, "Tove").map_err(|e| e.to_string())?;
                let first = x.sheets[0].id;
                x.sheet_mut(first).unwrap().elements = vec![net, tove];
                Ok((cast, props))
            })
            .unwrap();
        let count = |c: Option<u32>| LayoutColumnInput {
            category: c,
            ..col("element_count", "narrow")
        };
        let cols = [col("scene", "narrow"), count(Some(props.0))];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(v.board.layout[1].category, Some(props.0));
        let cells =
            |v: &crate::view::View, i: usize| v.board.sub_boards[0].days[0].strips[i].cells.clone();
        assert_eq!(cells(&v, 0), ["1", "Props (1)"]);
        assert_eq!(cells(&v, 1), ["2", "Props (0)"]);
        // Another pick replaces it; none counts every element, quiet at none.
        let cols = [col("scene", "narrow"), count(Some(cast.0))];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        assert_eq!(
            cells(&crate::view::view(&s, None).unwrap(), 0),
            ["1", "Cast (1)"]
        );
        let cols = [col("scene", "narrow"), count(None)];
        s.edit(|x| set_strip_layout(x, b, &cols, "")).unwrap();
        let v = crate::view::view(&s, None).unwrap();
        assert_eq!(cells(&v, 0), ["1", "2"]);
        assert_eq!(cells(&v, 1), ["2", ""]);
        // Refused: a category the schedule lacks, or one on another field.
        let before = s.schedule().clone();
        for bad in [
            vec![col("scene", "narrow"), count(Some(999_999))],
            vec![LayoutColumnInput {
                category: Some(props.0),
                ..col("scene", "narrow")
            }],
        ] {
            let e = s.edit(|x| set_strip_layout(x, b, &bad, "")).unwrap_err();
            assert!(e.ends_with('.') && !e.contains('!'), "{e}");
        }
        assert_eq!(s.schedule(), &before);
    }
}
