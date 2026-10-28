// SPDX-License-Identifier: Apache-2.0

//! Aggregated, bounded review warnings.
//!
//! Warnings describe what a spreadsheet application *might* change; they are
//! never assertions that a conversion occurred. Each warning keeps a small
//! sample and a count, so a 20-million-row file does not produce a warning per
//! row.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
    HeaderEmpty,
    HeaderDuplicate,
    RaggedRow,
    ColumnLimitReached,
    LeadingZeros,
    AmbiguousDate,
    MixedTypes,
    BooleanSpellings,
    LongNumber,
    FormulaLikeText,
    LinkLikeText,
    QuotedMultiline,
    LongField,
    ReplacementErrors,
}

impl WarningCode {
    pub fn severity(self) -> Severity {
        match self {
            WarningCode::QuotedMultiline
            | WarningCode::LinkLikeText
            | WarningCode::LongField
            | WarningCode::HeaderEmpty
            | WarningCode::HeaderDuplicate => Severity::Info,
            _ => Severity::Warning,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            WarningCode::HeaderEmpty => {
                "Header cell is empty; a generated column name is shown instead."
            }
            WarningCode::HeaderDuplicate => {
                "Duplicate header names were made unique for display only."
            }
            WarningCode::RaggedRow => {
                "Some rows have a different number of fields than the column set."
            }
            WarningCode::ColumnLimitReached => {
                "The column limit was reached; additional fields are not shown."
            }
            WarningCode::LeadingZeros => {
                "Values have leading zeros and may be treated as numbers by spreadsheet apps; source text is unchanged."
            }
            WarningCode::AmbiguousDate => {
                "Dates where day and month are both 12 or less can be read two ways; choose an interpretation explicitly."
            }
            WarningCode::MixedTypes => {
                "A column mixes value types; spreadsheet apps may coerce some values."
            }
            WarningCode::BooleanSpellings => {
                "A column mixes boolean spellings (for example true/yes/1); they may be interpreted inconsistently."
            }
            WarningCode::LongNumber => {
                "Numbers with more than 15 digits may lose precision in spreadsheet apps."
            }
            WarningCode::FormulaLikeText => {
                "Cells start with formula-like characters; they are rendered as text and never executed."
            }
            WarningCode::LinkLikeText => {
                "Link-like text is shown literally and is never opened."
            }
            WarningCode::QuotedMultiline => {
                "Quoted fields contain line breaks; they are preserved as text."
            }
            WarningCode::LongField => {
                "A field exceeded the display limit and was truncated for this report."
            }
            WarningCode::ReplacementErrors => {
                "Some bytes were not valid for the selected encoding and were replaced while decoding."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarningSample {
    /// 1-based data record, or 0 when the warning is not tied to a data row.
    pub record: u64,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: WarningCode,
    pub severity: Severity,
    pub column: Option<usize>,
    pub column_name: Option<String>,
    pub occurrences: u64,
    pub message: String,
    pub samples: Vec<WarningSample>,
}

const MAX_SAMPLES: usize = 3;
const MAX_SAMPLE_CHARS: usize = 200;

#[derive(Debug, Clone, Default)]
pub struct WarningCollector {
    entries: BTreeMap<(Option<usize>, WarningCode), Warning>,
}

impl WarningCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, column: Option<usize>, code: WarningCode, record: u64, value: &str) {
        self.add_with_message(column, code, record, value, None);
    }

    pub fn add_with_message(
        &mut self,
        column: Option<usize>,
        code: WarningCode,
        record: u64,
        value: &str,
        message: Option<String>,
    ) {
        self.add_count_with_message(column, code, 1, record, value, message);
    }

    /// Add an aggregated count without producing one sample per occurrence.
    pub fn add_count(
        &mut self,
        column: Option<usize>,
        code: WarningCode,
        occurrences: u64,
        record: u64,
        value: &str,
    ) {
        self.add_count_with_message(column, code, occurrences, record, value, None);
    }

    fn add_count_with_message(
        &mut self,
        column: Option<usize>,
        code: WarningCode,
        occurrences: u64,
        record: u64,
        value: &str,
        message: Option<String>,
    ) {
        let entry = self
            .entries
            .entry((column, code))
            .or_insert_with(|| Warning {
                code,
                severity: code.severity(),
                column,
                column_name: None,
                occurrences: 0,
                message: message
                    .clone()
                    .unwrap_or_else(|| code.message().to_string()),
                samples: Vec::new(),
            });
        entry.occurrences += occurrences;
        if let Some(message) = message {
            if entry.occurrences == occurrences {
                entry.message = message;
            }
        }
        if entry.samples.len() < MAX_SAMPLES && !value.is_empty() {
            entry.samples.push(WarningSample {
                record,
                value: sanitize_sample(value),
            });
        }
    }

    pub fn finish(mut self, column_names: &[String]) -> Vec<Warning> {
        for warning in self.entries.values_mut() {
            if let Some(column) = warning.column {
                warning.column_name = column_names.get(column).cloned();
            }
        }
        self.entries.into_values().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn sanitize_sample(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if result.chars().count() >= MAX_SAMPLE_CHARS {
            result.push('…');
            break;
        }
        match ch {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            ch if ch.is_control() => result.push('�'),
            ch => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_and_sorts_warnings() {
        let mut collector = WarningCollector::new();
        collector.add(Some(1), WarningCode::LeadingZeros, 1, "007");
        collector.add(Some(1), WarningCode::LeadingZeros, 2, "008");
        collector.add(Some(0), WarningCode::FormulaLikeText, 1, "=SUM(A1)");
        collector.add(None, WarningCode::ReplacementErrors, 0, "");
        let warnings = collector.finish(&["a".into(), "b".into()]);
        assert_eq!(warnings.len(), 3);
        assert_eq!(warnings[0].code, WarningCode::ReplacementErrors);
        assert_eq!(warnings[1].code, WarningCode::FormulaLikeText);
        assert_eq!(warnings[2].occurrences, 2);
        assert_eq!(warnings[2].column_name.as_deref(), Some("b"));
    }

    #[test]
    fn samples_are_bounded_and_escaped() {
        let mut collector = WarningCollector::new();
        for index in 0..10 {
            collector.add(
                Some(0),
                WarningCode::QuotedMultiline,
                index,
                "a\nb\tc",
            );
        }
        let warnings = collector.finish(&["x".into()]);
        assert_eq!(warnings[0].occurrences, 10);
        assert_eq!(warnings[0].samples.len(), MAX_SAMPLES);
        assert_eq!(warnings[0].samples[0].value, "a\\nb\\tc");
    }
}
