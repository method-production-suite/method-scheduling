//! The report list and the settings the schedule keeps for reports and the day out of
//! days (decision 59): each report's name, order and options, the production info, the
//! code letters and a sub-board's prep start and wrap end. Plus the user's own display
//! preferences (week start, date format, what a calendar strip shows), which live in the
//! app's `settings.json`, not in the schedule.
//!
//! Every schedule change is one undo step through `Session::edit`; options travel from
//! the page as plain values and are read strictly here (typed dates through
//! `calendar::typed_date`, names of fields, header layouts and sizes checked by name).

use serde::{Deserialize, Serialize};

use method_schedule_core::headers::{HeaderAlign, HeaderDesign, HeaderId, HeaderItem};
use method_schedule_core::layouts::{LayoutBody, LayoutId};
use method_schedule_core::model::{Align, CategoryId, ElementId, Schedule, StripField};
use method_schedule_core::setup::{
    PageBreak, ReportColumn, ReportOptions, ReportSetup, TextSize, NAME_LIMIT,
};

use crate::board::EditResult;
use crate::calendar::typed_date;
use crate::view::NamedDto;

/// One report of the list, as the Reports tab shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ReportItemDto {
    pub index: usize,
    pub name: String,
    /// The kind's name (`one_line`, `breakdown`...) and its usual title.
    pub kind: &'static str,
    pub kind_label: &'static str,
    pub landscape: bool,
    pub options: OptionsDto,
}

/// A report's options as the page edits them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptionsDto {
    /// Dates as typed; `""` for none.
    pub from: String,
    pub to: String,
    /// `None`: the kind's default.
    pub landscape: Option<bool>,
    pub synopsis: bool,
    pub cast_names: bool,
    /// `none`, `day` or `week`.
    pub page_break: String,
    pub shoot_days: bool,
    pub day_breaks: bool,
    pub total_count: bool,
    /// Category ids: a column each after the scene columns.
    pub element_columns: Vec<u32>,
    /// A named strip layout's id; `None`: the report's own columns.
    pub layout: Option<u32>,
    pub banners: bool,
    pub categories: Vec<u32>,
    pub elements: Vec<u32>,
    pub colors: bool,
    pub icons: bool,
    /// The header layout's id (decision 99).
    pub header: u32,
    pub text: String,
    pub columns: Vec<ColumnDto>,
    pub no_holds: bool,
    pub weekdays_only: bool,
}

/// One column of a report as the page edits it (the report designer, decisions.md, entry
/// 89): a field's name alone, or the field with its line (1 to 4), the first line's
/// columns it covers, its alignment, bold, italic and text size (`""`: the report's).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColumnDto {
    Field(String),
    Cell(CellDto),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CellDto {
    pub field: String,
    pub line: u8,
    pub span: u8,
    pub align: String,
    pub bold: bool,
    pub italic: bool,
    pub size: String,
}

impl Default for CellDto {
    fn default() -> CellDto {
        CellDto {
            field: String::new(),
            line: 1,
            span: 1,
            align: "auto".to_string(),
            bold: false,
            italic: false,
            size: String::new(),
        }
    }
}

impl ColumnDto {
    pub fn of(c: &ReportColumn) -> ColumnDto {
        ColumnDto::Cell(CellDto {
            field: c.field.name().to_string(),
            line: c.line + 1,
            span: c.span,
            align: c.align.name().to_string(),
            bold: c.bold,
            italic: c.italic,
            size: c.size.map_or(String::new(), |t| t.name().to_string()),
        })
    }

    /// The column, read strictly.
    pub fn read(&self) -> EditResult<ReportColumn> {
        let field = |f: &str| {
            StripField::from_name(f)
                .filter(|f| f.of_sheet())
                .ok_or_else(|| format!("{f:?} is not a field."))
        };
        let c = match self {
            ColumnDto::Field(f) => return Ok(ReportColumn::of(field(f)?)),
            ColumnDto::Cell(c) => c,
        };
        if !(1..=4).contains(&c.line) {
            return Err(format!(
                "Line {} is not a line of a report (1 to 4).",
                c.line
            ));
        }
        let align = Align::from_name(&c.align)
            .ok_or_else(|| format!("{:?} is not an alignment.", c.align))?;
        let size = match c.size.as_str() {
            "" => None,
            t => Some(TextSize::from_name(t).ok_or_else(|| format!("{t:?} is not a text size."))?),
        };
        Ok(ReportColumn {
            field: field(&c.field)?,
            line: c.line - 1,
            span: if c.line == 1 { 1 } else { c.span },
            align,
            bold: c.bold,
            italic: c.italic,
            size,
        })
    }
}

impl OptionsDto {
    pub fn of(o: &ReportOptions) -> OptionsDto {
        OptionsDto {
            from: o.from.map(|d| d.to_string()).unwrap_or_default(),
            to: o.to.map(|d| d.to_string()).unwrap_or_default(),
            landscape: o.landscape,
            synopsis: o.synopsis,
            cast_names: o.cast_names,
            page_break: o.page_break.name().to_string(),
            shoot_days: o.shoot_days,
            day_breaks: o.day_breaks,
            total_count: o.total_count,
            element_columns: o.element_columns.iter().map(|c| c.0).collect(),
            layout: o.layout.map(|l| l.0),
            banners: o.banners,
            categories: o.categories.iter().map(|c| c.0).collect(),
            elements: o.elements.iter().map(|e| e.0).collect(),
            colors: o.colors,
            icons: o.icons,
            header: o.header.0,
            text: o.text.name().to_string(),
            columns: o.columns.iter().map(ColumnDto::of).collect(),
            no_holds: o.no_holds,
            weekdays_only: o.weekdays_only,
        }
    }

    /// The options, read strictly.
    pub fn read(&self) -> EditResult<ReportOptions> {
        let date = |what: &str, v: &str| -> EditResult<Option<method_schedule_core::Date>> {
            let v = v.trim();
            if v.is_empty() {
                return Ok(None);
            }
            typed_date(v).map(Some).ok_or_else(|| {
                format!("The {what} date {v:?} is not a date. Type it as 3/1/2027 or 2027-03-01.")
            })
        };
        let from = date("From", &self.from)?;
        let to = date("To", &self.to)?;
        if let (Some(f), Some(t)) = (from, to) {
            if t < f {
                return Err(format!("The date range ends ({t}) before it starts ({f})."));
            }
        }
        let page_break = PageBreak::from_name(&self.page_break)
            .ok_or_else(|| format!("{:?} is not a page break.", self.page_break))?;
        let text = TextSize::from_name(&self.text)
            .ok_or_else(|| format!("{:?} is not a text size.", self.text))?;
        let columns = self
            .columns
            .iter()
            .map(ColumnDto::read)
            .collect::<Result<Vec<_>, _>>()?;
        if self.categories.len() > 500
            || self.elements.len() > 5000
            || columns.len() > 13
            || self.element_columns.len() > 50
        {
            return Err("Too many picked.".to_string());
        }
        Ok(ReportOptions {
            from,
            to,
            landscape: self.landscape,
            synopsis: self.synopsis,
            cast_names: self.cast_names,
            page_break,
            shoot_days: self.shoot_days,
            day_breaks: self.day_breaks,
            total_count: self.total_count,
            element_columns: self
                .element_columns
                .iter()
                .copied()
                .map(CategoryId)
                .collect(),
            layout: self.layout.map(LayoutId),
            banners: self.banners,
            categories: self.categories.iter().copied().map(CategoryId).collect(),
            elements: self.elements.iter().copied().map(ElementId).collect(),
            colors: self.colors,
            icons: self.icons,
            header: HeaderId(self.header),
            text,
            columns,
            no_holds: self.no_holds,
            weekdays_only: self.weekdays_only,
        })
    }
}

/// What the Reports tab needs to list and edit reports.
#[derive(Clone, Debug, Serialize)]
pub struct ReportsDto {
    pub reports: Vec<ReportItemDto>,
    pub title: String,
    pub company: String,
    pub director: String,
    pub producer: String,
    /// Every production info field in the dialog's order: `[key, label, value]`.
    pub production: Vec<(&'static str, &'static str, String)>,
    /// Every category, and every element by category (`ID NAME`), for the filters.
    pub categories: Vec<NamedDto>,
    pub elements: Vec<crate::view::ElementDto>,
    /// The scene fields a report can show, `[name, label]`, in order.
    pub fields: Vec<(&'static str, &'static str)>,
    /// The header layouts in order (decision 99), and what a header row can hold,
    /// `[name, label]` (custom text aside).
    pub headers: Vec<HeaderDto>,
    pub header_items: Vec<(&'static str, &'static str)>,
    /// The named strip layouts a report can follow.
    pub strip_layouts: Vec<NamedDto>,
}

/// A header layout as the page edits it: id 0 for a new one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderDto {
    pub id: u32,
    pub name: String,
    pub include: bool,
    pub first_page_only: bool,
    pub bold: bool,
    pub italic: bool,
    /// `left`, `center` or `right`.
    pub align: String,
    /// `small`, `normal` or `large`.
    pub size: String,
    pub rows: Vec<Vec<HeaderItemDto>>,
}

/// A header item: its name (`report_name`, `director`), or custom text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeaderItemDto {
    Name(String),
    Text(HeaderTextDto),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderTextDto {
    pub text: String,
}

impl HeaderDto {
    pub fn of(h: &HeaderDesign) -> HeaderDto {
        HeaderDto {
            id: h.id.0,
            name: h.name.clone(),
            include: h.include,
            first_page_only: h.first_page_only,
            bold: h.bold,
            italic: h.italic,
            align: h.align.name().to_string(),
            size: h.size.name().to_string(),
            rows: h
                .rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|i| match i {
                            HeaderItem::Text(t) => {
                                HeaderItemDto::Text(HeaderTextDto { text: t.clone() })
                            }
                            other => HeaderItemDto::Name(other.name().to_string()),
                        })
                        .collect()
                })
                .collect(),
        }
    }

    /// The layout, read strictly.
    pub fn read(&self) -> EditResult<HeaderDesign> {
        let align = HeaderAlign::from_name(&self.align)
            .ok_or_else(|| format!("{:?} is not an alignment.", self.align))?;
        let size = TextSize::from_name(&self.size)
            .ok_or_else(|| format!("{:?} is not a text size.", self.size))?;
        let rows = self
            .rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|i| match i {
                        HeaderItemDto::Text(t) => Ok(HeaderItem::Text(t.text.trim().to_string())),
                        HeaderItemDto::Name(n) => HeaderItem::from_name(n)
                            .ok_or_else(|| format!("{n:?} is not a header field.")),
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(HeaderDesign {
            id: HeaderId(self.id),
            name: self.name.clone(),
            include: self.include,
            first_page_only: self.first_page_only,
            bold: self.bold,
            italic: self.italic,
            align,
            size,
            rows,
        })
    }
}

pub fn reports_dto(s: &Schedule) -> ReportsDto {
    let reports = s
        .report_list()
        .iter()
        .enumerate()
        .map(|(index, r)| ReportItemDto {
            index,
            name: r.name.clone(),
            kind: r.kind.name(),
            kind_label: r.kind.label(),
            landscape: r.landscape(),
            options: OptionsDto::of(&r.options),
        })
        .collect();
    let mut elements = Vec::new();
    for c in &s.categories {
        let mut els: Vec<_> = s.elements.iter().filter(|e| e.category == c.id).collect();
        els.sort_by(|a, b| {
            method_schedule_core::reports::id_order(a.board_id.as_deref(), b.board_id.as_deref())
                .then_with(|| {
                    a.name
                        .to_ascii_uppercase()
                        .cmp(&b.name.to_ascii_uppercase())
                })
        });
        elements.extend(els.into_iter().map(|e| crate::view::ElementDto {
            id: e.id.0,
            name: e.name.clone(),
            board_id: e.board_id.clone().unwrap_or_default(),
            category: c.name.clone(),
            quantity: None,
            daily: None,
        }));
    }
    ReportsDto {
        reports,
        title: s.title.clone(),
        company: s.production.company.clone(),
        director: s.production.director.clone(),
        producer: s.production.producer.clone(),
        production: method_schedule_core::setup::Production::FIELDS
            .iter()
            .map(|f| {
                (
                    *f,
                    method_schedule_core::setup::Production::label(f),
                    s.production.get(f).unwrap_or_default().to_string(),
                )
            })
            .collect(),
        categories: s
            .categories
            .iter()
            .map(|c| NamedDto {
                id: c.id.0,
                name: c.name.clone(),
            })
            .collect(),
        elements,
        fields: StripField::SHEET
            .iter()
            .map(|f| (f.name(), f.label()))
            .collect(),
        headers: s.header_list().iter().map(HeaderDto::of).collect(),
        header_items: HeaderItem::all()
            .iter()
            .map(|i| (i.name(), i.label()))
            .collect(),
        strip_layouts: s
            .layouts
            .layouts
            .iter()
            .filter(|l| matches!(l.body, LayoutBody::Strips(_)))
            .map(|l| NamedDto {
                id: l.id.0,
                name: l.name.clone(),
            })
            .collect(),
    }
}

pub fn say(e: method_schedule_core::model::Error) -> String {
    use method_schedule_core::model::Error;
    match e {
        Error::DuplicateName(n) => format!("A report named {n:?} is already in the list."),
        Error::InUse(why) => format!("{} can't be deleted: it is the last of its kind.", {
            why.split(" is the only").next().unwrap_or(&why).to_string()
        }),
        Error::Broken(why) => {
            let mut c = why.chars();
            match c.next() {
                Some(f) => format!("{}{}.", f.to_uppercase(), c.as_str()),
                None => "That change was refused.".to_string(),
            }
        }
        other => other.to_string(),
    }
}

/// Saves report `index` with `name` and `options`.
pub fn apply_report(
    s: &mut Schedule,
    index: usize,
    name: &str,
    options: &OptionsDto,
) -> EditResult {
    let list = s.report_list();
    let old = list
        .get(index)
        .ok_or("That report is no longer in the list.")?;
    let name = name.trim();
    if name.is_empty() {
        return Err("A report needs a name.".to_string());
    }
    if name.chars().count() > NAME_LIMIT {
        return Err(format!("A report name is {NAME_LIMIT} characters at most."));
    }
    let setup = ReportSetup {
        name: name.to_string(),
        kind: old.kind,
        options: options.read()?,
    };
    // A header layout picked here must be in the list (one deleted since stays picked
    // only on reports that had it).
    if setup.options.header != old.options.header
        && !s.header_list().iter().any(|h| h.id == setup.options.header)
    {
        return Err("That header layout is no longer in the list.".to_string());
    }
    s.set_report(index, setup).map_err(say)
}

/// Saves header layout `design`: a new one at the end when its id is 0, else in place.
pub fn apply_header(s: &mut Schedule, design: &HeaderDto) -> EditResult<u32> {
    let mut h = design.read()?;
    h.name = h.name.trim().to_string();
    if h.name.is_empty() {
        return Err("A header layout needs a name.".to_string());
    }
    if h.id.0 == 0 {
        return s.add_header(h).map(|id| id.0).map_err(header_say);
    }
    let id = h.id.0;
    s.set_header(h).map_err(header_say)?;
    Ok(id)
}

pub fn header_say(e: method_schedule_core::model::Error) -> String {
    use method_schedule_core::model::Error;
    match e {
        Error::DuplicateName(n) => format!("A header layout named {n:?} is already in the list."),
        Error::InUse(_) => "The last header layout can't be deleted.".to_string(),
        other => say(other),
    }
}

pub fn apply_production(s: &mut Schedule, field: &str, value: &str) -> EditResult {
    let value: String = value.trim().chars().take(200).collect();
    if field == "title" {
        if value.is_empty() {
            return Err("The schedule needs a title.".to_string());
        }
        s.title = value;
        return Ok(());
    }
    s.production
        .set(field, &value)
        .map_err(|_| format!("{field:?} is not a production field."))
}

pub fn apply_codes(
    s: &mut Schedule,
    codes: &std::collections::BTreeMap<String, String>,
) -> EditResult {
    let mut c = s.dood_codes.clone();
    for (k, v) in codes {
        let slot = c
            .get_mut(k)
            .ok_or_else(|| format!("{k:?} is not a day out of days code."))?;
        *slot = v.trim().to_string();
    }
    if let Some(why) = c.problem() {
        let mut ch = why.chars();
        let first = ch
            .next()
            .map(|f| f.to_uppercase().to_string())
            .unwrap_or_default();
        return Err(format!("{first}{}.", ch.as_str()));
    }
    s.dood_codes = c;
    Ok(())
}

/// Sets a sub-board's prep start and wrap end, as typed (empty clears).
pub fn apply_prep_wrap(s: &mut Schedule, sub: u32, prep: &str, wrap: &str) -> EditResult {
    let read = |what: &str, v: &str| -> EditResult<Option<method_schedule_core::Date>> {
        let v = v.trim();
        if v.is_empty() {
            return Ok(None);
        }
        typed_date(v).map(Some).ok_or_else(|| {
            format!("The {what} {v:?} is not a date. Type it as 3/1/2027 or 2027-03-01.")
        })
    };
    let (p, w) = (read("prep start", prep)?, read("wrap end", wrap)?);
    if let (Some(p), Some(w)) = (p, w) {
        if w < p {
            return Err(format!("Wrap can't end ({w}) before prep starts ({p})."));
        }
    }
    let (b, sb) = {
        let (b, sb) = crate::view::sub_ref(s, sub)?;
        (b.id, sb.id)
    };
    let cal = s.calendar_mut(b, sb).map_err(|e| e.to_string())?;
    cal.prep_start = p;
    cal.wrap_end = w;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands

/// Whether `old`, the user's per-user calendar fields from before they were kept in the
/// schedule, is a pick of the user's own (not empty, not that preference's default of
/// scene, set and pages), so the Calendar starts from it (decision 102).
pub fn old_calendar_pick(old: &[StripField]) -> bool {
    !old.is_empty()
        && old.iter().enumerate().all(|(i, f)| !old[..i].contains(f))
        && old != [StripField::Scene, StripField::Set, StripField::Pages]
}

/// Sets what a calendar strip shows, by field name.
pub fn apply_calendar_fields(s: &mut Schedule, fields: &[String]) -> EditResult {
    if fields.len() > method_schedule_core::model::MAX_CALENDAR_FIELDS {
        return Err("A calendar strip shows 13 fields at most.".to_string());
    }
    let fields = fields
        .iter()
        .map(|x| {
            StripField::from_name(x)
                .filter(|f| f.of_sheet())
                .ok_or_else(|| format!("{x:?} is not a field."))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if fields.is_empty() {
        return Err("A calendar strip shows at least one field.".to_string());
    }
    s.set_calendar_fields(fields)
        .map_err(|_| "Each field can show once on a calendar strip.".to_string())
}
// ---------------------------------------------------------------------------
// The user's preferences (settings.json; not in the schedule)

/// How the user likes dates and the calendar shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// `sun` or `mon`.
    pub week_start: String,
    /// `iso` (2027-03-01), `us` (3/1/2027) or `long` (Mon 1 Mar 2027).
    pub date_format: String,
    /// What a strip shows on a calendar date: strip layout fields, in order.
    pub calendar_fields: Vec<String>,
    /// Lines on a page when a Fountain script is imported, 40 to 70 (decision 195).
    pub fountain_lines: u32,
    /// The app's look (decision 208): `paper` (the default, light or dark with the
    /// system), `graphite` (always dark) or `ledger` (always light, full strip colors).
    pub theme: String,
}

/// The looks the app has (decision 208), the default first.
pub const THEMES: [&str; 3] = ["paper", "graphite", "ledger"];

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            week_start: "sun".to_string(),
            date_format: "iso".to_string(),
            calendar_fields: vec!["scene".into(), "set".into(), "pages".into()],
            fountain_lines: method_schedule_core::fountain::LINES_PER_PAGE,
            theme: THEMES[0].to_string(),
        }
    }
}

impl Prefs {
    /// The preferences, read strictly.
    pub fn checked(&self) -> Result<Prefs, String> {
        if !["sun", "mon"].contains(&self.week_start.as_str()) {
            return Err(format!(
                "{:?} is not a week start (sun or mon).",
                self.week_start
            ));
        }
        if !["iso", "us", "long"].contains(&self.date_format.as_str()) {
            return Err(format!("{:?} is not a date format.", self.date_format));
        }
        let mut seen = Vec::new();
        for f in &self.calendar_fields {
            if !StripField::from_name(f).is_some_and(|f| f.of_sheet()) {
                return Err(format!("{f:?} is not a field."));
            }
            if seen.contains(f) {
                return Err(format!("{f:?} is shown twice."));
            }
            seen.push(f.clone());
        }
        if seen.is_empty() {
            return Err("A calendar strip shows at least one field.".to_string());
        }
        use method_schedule_core::fountain::{MAX_LINES_PER_PAGE, MIN_LINES_PER_PAGE};
        if !(MIN_LINES_PER_PAGE..=MAX_LINES_PER_PAGE).contains(&self.fountain_lines) {
            return Err(format!(
                "Fountain lines per page are {MIN_LINES_PER_PAGE} to {MAX_LINES_PER_PAGE}."
            ));
        }
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(format!("{:?} is not a look the app has.", self.theme));
        }
        Ok(self.clone())
    }
}
