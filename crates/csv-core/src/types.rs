// SPDX-License-Identifier: Apache-2.0

//! Cell classification and deliberately small date parsing.
//!
//! The viewer never converts cells: values are kept as source text. These
//! helpers only *describe* text so warnings and date ranges can be offered
//! without silently changing the source.

use serde::{Deserialize, Serialize};

/// The coarse type of a single source cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellKind {
    Empty,
    Integer,
    Float,
    Boolean,
    Date,
    Text,
}

impl CellKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CellKind::Empty => "empty",
            CellKind::Integer => "integer",
            CellKind::Float => "float",
            CellKind::Boolean => "boolean",
            CellKind::Date => "date",
            CellKind::Text => "text",
        }
    }
}

/// A date interpretation explicitly chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateFormat {
    /// `YYYY-MM-DD`, `YYYY/MM/DD`, `YYYY.MM.DD`.
    Ymd,
    /// `DD-MM-YYYY`, `DD/MM/YYYY`, `DD.MM.YYYY`.
    Dmy,
    /// `MM-DD-YYYY`, `MM/DD/YYYY`, `MM.DD.YYYY`.
    Mdy,
}

impl DateFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            DateFormat::Ymd => "ymd",
            DateFormat::Dmy => "dmy",
            DateFormat::Mdy => "mdy",
        }
    }

    /// A short human description shown next to a computed range.
    pub fn description(self) -> &'static str {
        match self {
            DateFormat::Ymd => "year-month-day",
            DateFormat::Dmy => "day-month-year",
            DateFormat::Mdy => "month-day-year",
        }
    }
}

/// A validated civil date. Four-digit years only: two-digit years are
/// ambiguous and are never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CivilDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl CivilDate {
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let max = days_in_month(year, month);
        if day == 0 || day > max {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

pub fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Parse a date under an explicitly supplied interpretation.
pub fn parse_date(raw: &str, format: DateFormat) -> Option<CivilDate> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 32 {
        return None;
    }
    let mut separator = None;
    for ch in raw.chars() {
        if matches!(ch, '-' | '/' | '.') {
            if separator.is_some() && separator != Some(ch) {
                return None;
            }
            separator = Some(ch);
        } else if !ch.is_ascii_digit() {
            return None;
        }
    }
    let separator = separator?;
    let parts: Vec<&str> = raw.split(separator).collect();
    if parts.len() != 3 {
        return None;
    }
    let nums: Vec<u32> = parts
        .iter()
        .map(|part| {
            if part.is_empty() || part.len() > 4 {
                None
            } else {
                part.parse::<u32>().ok()
            }
        })
        .collect::<Option<Vec<_>>>()?;
    let (year, month, day) = match format {
        DateFormat::Ymd => {
            if parts[0].len() != 4 {
                return None;
            }
            (nums[0], nums[1], nums[2])
        }
        DateFormat::Dmy => {
            if parts[2].len() != 4 {
                return None;
            }
            (nums[2], nums[1], nums[0])
        }
        DateFormat::Mdy => {
            if parts[2].len() != 4 {
                return None;
            }
            (nums[2], nums[0], nums[1])
        }
    };
    if year > u16::MAX as u32 || month > u8::MAX as u32 || day > u8::MAX as u32 {
        return None;
    }
    CivilDate::new(year as u16, month as u8, day as u8)
}

/// Describe a date-like string without choosing a day/month order.
pub fn classify_date(raw: &str) -> Option<DateGuess> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 32 {
        return None;
    }
    let separator = raw.chars().find(|ch| matches!(ch, '-' | '/' | '.'))?;
    let parts: Vec<&str> = raw.split(separator).collect();
    if parts.len() != 3 {
        return None;
    }
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let nums: Vec<u32> = parts
        .iter()
        .map(|part| part.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;

    if parts[0].len() == 4 {
        let date = CivilDate::new(nums[0] as u16, nums[1] as u8, nums[2] as u8)?;
        return Some(DateGuess {
            format: DateFormat::Ymd,
            date,
            ambiguous: false,
        });
    }

    let year_len = parts[2].len();
    if year_len != 4 {
        return None;
    }
    let year = nums[2] as u16;
    // Both readings must be valid for the result to be genuinely ambiguous.
    let dmy = CivilDate::new(year, nums[1] as u8, nums[0] as u8);
    let mdy = CivilDate::new(year, nums[0] as u8, nums[1] as u8);
    match (dmy, mdy) {
        (Some(a), Some(b)) => Some(DateGuess {
            // When both readings are valid the `ambiguous` flag is what
            // matters; the report never silently commits to an order.
            format: DateFormat::Dmy,
            date: a,
            ambiguous: a != b,
        }),
        (Some(a), None) => Some(DateGuess {
            format: DateFormat::Dmy,
            date: a,
            ambiguous: false,
        }),
        (None, Some(b)) => Some(DateGuess {
            format: DateFormat::Mdy,
            date: b,
            ambiguous: false,
        }),
        (None, None) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateGuess {
    pub format: DateFormat,
    pub date: CivilDate,
    pub ambiguous: bool,
}

/// True when the only question is day/month order (e.g. `01/02/2024`).
pub fn is_ambiguous_date(raw: &str) -> bool {
    classify_date(raw)
        .map(|guess| guess.ambiguous)
        .unwrap_or(false)
}

/// Classify a cell without changing it.
pub fn classify_cell(raw: &str) -> CellKind {
    let text = raw.trim();
    if text.is_empty() {
        return CellKind::Empty;
    }
    if is_integer(text) {
        return CellKind::Integer;
    }
    if is_float(text) {
        return CellKind::Float;
    }
    if boolean_spelling(text).is_some() {
        return CellKind::Boolean;
    }
    if classify_date(text).is_some() {
        return CellKind::Date;
    }
    CellKind::Text
}

pub fn is_integer(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut start = 0;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        start = 1;
    }
    if start == bytes.len() {
        return false;
    }
    bytes[start..].iter().all(|b| b.is_ascii_digit())
}

pub fn is_float(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("nan") || lower.contains("inf") {
        return false;
    }
    if !(lower.contains('.') || lower.contains('e')) {
        return false;
    }
    text.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false)
}

/// Canonical spelling of a boolean word. `0` and `1` are intentionally not
/// included: they are numbers first, and are reported separately as
/// potentially mixed boolean spellings.
pub fn boolean_spelling(text: &str) -> Option<&'static str> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" => Some("true"),
        "false" => Some("false"),
        "yes" => Some("yes"),
        "no" => Some("no"),
        "y" => Some("y"),
        "n" => Some("n"),
        "t" => Some("t"),
        "f" => Some("f"),
        _ => None,
    }
}

/// Number of decimal digits in an integer-looking value, ignoring sign and
/// grouping-free punctuation. Used to flag values Excel may round.
pub fn digit_count(text: &str) -> usize {
    text.trim().chars().filter(|c| c.is_ascii_digit()).count()
}

/// True when a value would be interpreted as a formula or command by a
/// spreadsheet application. We never execute these; the warning lets the user
/// know the source contains them.
pub fn is_formula_like(raw: &str) -> bool {
    let trimmed = raw.trim_start_matches([' ', '\t']);
    let mut chars = trimmed.chars();
    match chars.next() {
        Some('=') | Some('@') => true,
        Some('+') | Some('-') => {
            let rest = &trimmed[1..];
            let mut rest_chars = rest.chars();
            match rest_chars.next() {
                Some(next) if next.is_ascii_digit() || next == '.' => {
                    // A plain signed number is data. A signed expression such
                    // as `-2+3` still looks like a formula to a spreadsheet.
                    rest.chars()
                        .find(|ch| !(ch.is_ascii_digit() || *ch == '.'))
                        .map(|ch| {
                            matches!(
                                ch,
                                '+' | '-' | '*' | '/' | '^' | '&' | '%' | '(' | ')' | ':'
                            )
                        })
                        .unwrap_or(false)
                }
                Some(_) => true,
                None => false,
            }
        }
        Some('\t') | Some('\r') | Some('\u{feff}') => true,
        _ => false,
    }
}

/// True for text that a UI might be tempted to turn into a clickable link.
/// The viewer always shows it as text.
pub fn is_link_like(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("file://")
        || lower.contains("www.")
        || lower.contains("mailto:")
}

/// True when a quoted field carried an embedded line break.
pub fn has_embedded_newline(raw: &str) -> bool {
    raw.contains(['\n', '\r'])
}

/// True when an integer-looking value carries a leading zero (`007`, `0012`).
/// A bare `0` and decimal/exponent forms are excluded.
pub fn has_leading_zero(raw: &str) -> bool {
    let text = raw.trim();
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    digits.len() > 1
        && digits.starts_with('0')
        && digits.as_bytes()[1].is_ascii_digit()
        && digits.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_basic_cells() {
        assert_eq!(classify_cell(""), CellKind::Empty);
        assert_eq!(classify_cell("   "), CellKind::Empty);
        assert_eq!(classify_cell("42"), CellKind::Integer);
        assert_eq!(classify_cell("-42"), CellKind::Integer);
        assert_eq!(classify_cell("3.14"), CellKind::Float);
        assert_eq!(classify_cell("1e6"), CellKind::Float);
        assert_eq!(classify_cell("true"), CellKind::Boolean);
        assert_eq!(classify_cell("2024-01-31"), CellKind::Date);
        assert_eq!(classify_cell("hello"), CellKind::Text);
        assert_eq!(classify_cell("1,5"), CellKind::Text);
        assert_eq!(classify_cell("NaN"), CellKind::Text);
    }

    #[test]
    fn flags_ambiguity_only_when_day_and_month_both_possible() {
        assert!(is_ambiguous_date("01/02/2024"));
        assert!(is_ambiguous_date("12.11.2024"));
        assert!(!is_ambiguous_date("2024-01-02"));
        assert!(!is_ambiguous_date("13/02/2024"));
        assert!(!is_ambiguous_date("02/13/2024"));
        assert!(!is_ambiguous_date("31/02/2024"));
        assert!(!is_ambiguous_date("not a date"));
    }

    #[test]
    fn parses_explicit_interpretations() {
        assert_eq!(
            parse_date("2024-02-29", DateFormat::Ymd),
            CivilDate::new(2024, 2, 29)
        );
        assert_eq!(
            parse_date("29/02/2024", DateFormat::Dmy),
            CivilDate::new(2024, 2, 29)
        );
        assert_eq!(
            parse_date("02/29/2024", DateFormat::Mdy),
            CivilDate::new(2024, 2, 29)
        );
        assert_eq!(parse_date("2023-02-29", DateFormat::Ymd), None);
        assert_eq!(parse_date("29/02/24", DateFormat::Dmy), None);
        assert_eq!(
            parse_date("2024-2-9", DateFormat::Ymd),
            Some(CivilDate {
                year: 2024,
                month: 2,
                day: 9
            })
        );
    }

    #[test]
    fn formula_detection_keeps_negative_numbers() {
        assert!(is_formula_like("=SUM(A1:A5)"));
        assert!(is_formula_like("+cmd|' /C calc'!A0"));
        assert!(is_formula_like("-2+3"));
        assert!(is_formula_like("@SUM(A1)"));
        assert!(!is_formula_like("-42"));
        assert!(!is_formula_like("+1.5"));
        assert!(!is_formula_like("plain"));
    }

    #[test]
    fn leading_zero_and_links() {
        assert!(has_leading_zero("007"));
        assert!(has_leading_zero("+0012"));
        assert!(!has_leading_zero("0"));
        assert!(!has_leading_zero("0.5"));
        assert!(!has_leading_zero("10"));
        assert!(is_link_like("HTTPS://example.invalid/x"));
        assert!(!is_link_like("not a link"));
    }
}
