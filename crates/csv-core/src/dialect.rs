// SPDX-License-Identifier: Apache-2.0

//! Delimiter detection and header inference.
//!
//! Detection is a heuristic over a bounded sample. Every choice can be
//! overridden; the report always says whether a value was detected or chosen.

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};

use crate::encoding::ChoiceSource;
use crate::types::{classify_cell, CellKind};

pub const DELIMITER_CANDIDATES: [char; 4] = [',', ';', '\t', '|'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dialect {
    pub delimiter: char,
    pub quote: char,
    pub double_quote: bool,
    pub flexible: bool,
    /// Optional escape character. `None` means RFC 4180 doubled quotes only.
    pub escape: Option<char>,
}

impl Default for Dialect {
    fn default() -> Self {
        Self {
            delimiter: ',',
            quote: '"',
            double_quote: true,
            flexible: true,
            escape: None,
        }
    }
}

impl Dialect {
    pub fn validate(&self) -> Result<(), String> {
        if !self.delimiter.is_ascii()
            || (self.delimiter.is_ascii_control() && self.delimiter != '\t')
        {
            return Err("delimiter must be printable ASCII (tab is allowed)".to_string());
        }
        if !self.quote.is_ascii() || self.quote.is_ascii_control() {
            return Err("quote must be a printable ASCII character".to_string());
        }
        if self.delimiter == self.quote {
            return Err("delimiter and quote must differ".to_string());
        }
        if let Some(escape) = self.escape {
            if !escape.is_ascii() || escape.is_ascii_control() || escape == self.delimiter {
                return Err("escape must be printable ASCII and differ from delimiter".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "dialect", rename_all = "lowercase")]
pub enum DialectChoice {
    Auto,
    Explicit(Dialect),
}

impl Default for DialectChoice {
    fn default() -> Self {
        DialectChoice::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialectReport {
    pub delimiter: char,
    pub quote: char,
    pub source: ChoiceSource,
    /// 0-100; detected confidence. Overrides report 100.
    pub confidence: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialectDetection {
    pub dialect: Dialect,
    pub confidence: u8,
}

/// Score delimiter candidates against a decoded sample.
pub fn detect(text: &str) -> DialectDetection {
    let mut best: Option<(f64, usize, DialectDetection)> = None;
    for (index, delimiter) in DELIMITER_CANDIDATES.iter().enumerate() {
        let dialect = Dialect {
            delimiter: *delimiter,
            ..Dialect::default()
        };
        if let Some((score, confidence)) = score_dialect(text, &dialect) {
            let adjusted = score + delimiter_bonus(text, *delimiter);
            let candidate = (
                adjusted,
                index,
                DialectDetection {
                    dialect,
                    confidence,
                },
            );
            if best
                .as_ref()
                .map(|(best_score, best_index, _)| {
                    adjusted > *best_score || (adjusted == *best_score && index < *best_index)
                })
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
    }
    match best {
        Some((_, _, detection)) if detection.confidence > 0 => detection,
        _ => DialectDetection {
            dialect: Dialect::default(),
            confidence: 0,
        },
    }
}

fn score_dialect(text: &str, dialect: &Dialect) -> Option<(f64, u8)> {
    if text.is_empty() {
        return Some((0.0, 0));
    }
    let rows = read_sample_records(text, dialect, 200);
    if rows.is_empty() {
        return Some((0.0, 0));
    }
    let mut counts = Vec::with_capacity(rows.len());
    for row in &rows {
        counts.push(row.len());
    }
    let mut frequency: Vec<(usize, usize)> = Vec::new();
    for count in &counts {
        match frequency.iter_mut().find(|(value, _)| value == count) {
            Some(entry) => entry.1 += 1,
            None => frequency.push((*count, 1)),
        }
    }
    frequency.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let (modal, modal_count) = *frequency.first()?;
    if modal < 2 {
        return Some((0.0, 0));
    }
    let consistency = modal_count as f64 / counts.len() as f64;
    let score = consistency * 1000.0 + (modal.min(50) as f64) * 2.0 + counts.len().min(100) as f64;
    Some((score, (consistency * 100.0).round().clamp(0.0, 100.0) as u8))
}

/// Prefer semicolon/tab over comma for European files where commas are decimal
/// separators (`1,5;2,5`). This is a heuristic and the user can always
/// override the dialect.
fn delimiter_bonus(text: &str, delimiter: char) -> f64 {
    if delimiter == ';' || delimiter == '\t' {
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(50)
            .collect();
        if lines.is_empty() {
            return 0.0;
        }
        let with_delimiter = lines.iter().filter(|line| line.contains(delimiter)).count();
        if with_delimiter * 10 < lines.len() * 8 {
            return 0.0;
        }
        let commas = text.matches(',').count();
        let decimal_commas = decimal_comma_count(text);
        if commas > 0 && decimal_commas * 10 >= commas * 8 {
            return 25.0;
        }
    }
    0.0
}

fn decimal_comma_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    for index in 0..bytes.len() {
        if bytes[index] == b',' {
            let before = index > 0 && bytes[index - 1].is_ascii_digit();
            let after = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
            if before && after {
                count += 1;
            }
        }
    }
    count
}

/// Parse a bounded number of records from a decoded sample.
pub fn read_sample_records(text: &str, dialect: &Dialect, max_records: usize) -> Vec<Vec<String>> {
    let mut builder = ReaderBuilder::new();
    builder
        .delimiter(dialect.delimiter as u8)
        .quote(dialect.quote as u8)
        .double_quote(dialect.double_quote)
        .flexible(dialect.flexible)
        .has_headers(false)
        .buffer_capacity(32 * 1024);
    if let Some(escape) = dialect.escape {
        builder.escape(Some(escape as u8));
    }
    let mut reader = builder.from_reader(text.as_bytes());
    let mut rows = Vec::new();
    let mut record = csv::StringRecord::new();
    for _ in 0..max_records {
        match reader.read_record(&mut record) {
            Ok(true) => rows.push(record.iter().map(|cell| cell.to_string()).collect()),
            Ok(false) | Err(_) => break,
        }
    }
    rows
}

/// Conservative header inference. A header is reported only when the first
/// row looks like labels and at least one column changes type on the next
/// rows. Text-only first rows stay data; the user can override.
pub fn detect_header(rows: &[Vec<String>]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let first = &rows[0];
    if first.len() < 2 || first.iter().any(|cell| cell.trim().is_empty()) || first.len() > 4096 {
        return false;
    }
    let unique: std::collections::BTreeSet<&str> = first.iter().map(|cell| cell.trim()).collect();
    if unique.len() != first.len() {
        return false;
    }

    let mut text_first = 0usize;
    let mut typed_difference = 0usize;
    for (column, label) in first.iter().enumerate() {
        let label_kind = classify_cell(label);
        if label_kind == CellKind::Text {
            text_first += 1;
        }
        let mut kinds: Vec<CellKind> = rows
            .iter()
            .skip(1)
            .take(50)
            .filter_map(|row| row.get(column))
            .map(|cell| classify_cell(cell))
            .filter(|kind| *kind != CellKind::Empty)
            .collect();
        if kinds.is_empty() {
            continue;
        }
        kinds.sort_by_key(|kind| *kind as u8);
        let modal = kinds[kinds.len() / 2];
        let differs =
            modal != label_kind && !(modal == CellKind::Text && label_kind == CellKind::Text);
        if differs {
            typed_difference += 1;
        }
    }
    let text_share = text_first * 2 >= first.len();
    let difference_share = typed_difference * 3 >= first.len();
    text_share && difference_share
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_delimiters() {
        assert_eq!(detect("a,b\n1,2\n").dialect.delimiter, ',');
        assert_eq!(detect("a;b\n1;2\n").dialect.delimiter, ';');
        assert_eq!(detect("a\tb\n1\t2\n").dialect.delimiter, '\t');
        assert_eq!(detect("a|b\n1|2\n").dialect.delimiter, '|');
        assert_eq!(detect("single\ncolumn\n").dialect.delimiter, ',');
    }

    #[test]
    fn quoted_delimiters_do_not_win() {
        let text = "name,note\nAda,\"hello, world\"\nGrace,\"a,b,c\"\n";
        assert_eq!(detect(text).dialect.delimiter, ',');
    }

    #[test]
    fn decimal_comma_prefers_semicolon() {
        let text = "date;open;close\n2024-01-01;1,5;1,7\n2024-01-02;1,6;1,8\n";
        assert_eq!(detect(text).dialect.delimiter, ';');
    }

    #[test]
    fn header_inference_is_conservative() {
        let rows = vec![
            vec!["id".into(), "name".into(), "age".into()],
            vec!["1".into(), "Ada".into(), "36".into()],
        ];
        assert!(detect_header(&rows));

        let rows = vec![
            vec!["Alice".into(), "Paris".into()],
            vec!["Bob".into(), "Rome".into()],
        ];
        assert!(!detect_header(&rows));

        let rows = vec![vec!["1".into(), "2".into()], vec!["3".into(), "4".into()]];
        assert!(!detect_header(&rows));
    }

    #[test]
    fn sample_parser_handles_quoted_newlines() {
        let rows = read_sample_records("a,b\n1,\"two\nlines\"\n", &Dialect::default(), 10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1], "two\nlines");
    }

    #[test]
    fn invalid_dialects_are_rejected() {
        assert!(Dialect::default().validate().is_ok());
        assert!(Dialect {
            delimiter: ',',
            quote: ',',
            ..Dialect::default()
        }
        .validate()
        .is_err());
    }
}
