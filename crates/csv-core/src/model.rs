// SPDX-License-Identifier: Apache-2.0

//! Scan request, coverage and result model.
//!
//! The central invariant is simple and deliberately visible: an exact number
//! of records is only reported when the scan actually reached the end of the
//! source. Incomplete scans carry a reason and no total.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::dialect::{DialectChoice, DialectReport};
use crate::encoding::{ChoiceSource, EncodingKind, EncodingReport};
use crate::market::{MarketReport, MarketRequest};
use crate::types::{CellKind, DateFormat};
use crate::warnings::Warning;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "encoding", rename_all = "lowercase")]
pub enum EncodingChoice {
    Auto,
    Explicit(EncodingKind),
}

impl Default for EncodingChoice {
    fn default() -> Self {
        EncodingChoice::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeaderChoice {
    Auto,
    Present,
    Absent,
}

impl Default for HeaderChoice {
    fn default() -> Self {
        HeaderChoice::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanLimits {
    pub max_records: Option<u64>,
    pub max_bytes: Option<u64>,
    pub deadline_ms: Option<u64>,
    pub max_field_chars: usize,
    pub max_columns: usize,
    /// Bound on all preview cell characters in the report. This keeps the
    /// worker protocol frame bounded even for wide, long-valued files.
    pub max_preview_chars: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_records: None,
            max_bytes: None,
            deadline_ms: None,
            max_field_chars: 4096,
            max_columns: 1024,
            max_preview_chars: 1024 * 1024,
        }
    }
}

impl ScanLimits {
    /// A short scan suitable for the first screen of a preview.
    pub fn preview() -> Self {
        Self {
            max_records: Some(200),
            max_bytes: Some(256 * 1024),
            deadline_ms: Some(5_000),
            max_field_chars: 2048,
            max_columns: 512,
            max_preview_chars: 512 * 1024,
        }
    }

    /// An explicitly requested complete scan, still bounded so a worker
    /// cannot run forever or consume unbounded memory.
    pub fn full() -> Self {
        Self {
            max_records: Some(100_000_000),
            max_bytes: Some(4 * 1024 * 1024 * 1024),
            deadline_ms: Some(120_000),
            max_field_chars: 4096,
            max_columns: 1024,
            max_preview_chars: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateRangeRequest {
    pub column: usize,
    pub format: DateFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanRequest {
    pub dialect: DialectChoice,
    pub encoding: EncodingChoice,
    pub header: HeaderChoice,
    pub limits: ScanLimits,
    pub preview_rows: usize,
    pub date_ranges: Vec<DateRangeRequest>,
    pub market: Option<MarketRequest>,
}

impl Default for ScanRequest {
    fn default() -> Self {
        Self {
            dialect: DialectChoice::Auto,
            encoding: EncodingChoice::Auto,
            header: HeaderChoice::Auto,
            limits: ScanLimits::default(),
            preview_rows: 200,
            date_ranges: Vec::new(),
            market: None,
        }
    }
}

impl ScanRequest {
    pub fn preview() -> Self {
        Self {
            limits: ScanLimits::preview(),
            preview_rows: 200,
            ..Self::default()
        }
    }

    pub fn full() -> Self {
        Self {
            limits: ScanLimits::full(),
            preview_rows: 100,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), ScanError> {
        if self.preview_rows == 0 {
            return Err(ScanError::InvalidRequest(
                "preview_rows must be greater than zero".to_string(),
            ));
        }
        if self.limits.max_field_chars == 0 {
            return Err(ScanError::InvalidRequest(
                "max_field_chars must be greater than zero".to_string(),
            ));
        }
        if self.limits.max_columns == 0 {
            return Err(ScanError::InvalidRequest(
                "max_columns must be greater than zero".to_string(),
            ));
        }
        if self.limits.max_preview_chars < 1024 {
            return Err(ScanError::InvalidRequest(
                "max_preview_chars must be at least 1024".to_string(),
            ));
        }
        if let DialectChoice::Explicit(dialect) = &self.dialect {
            dialect.validate().map_err(ScanError::InvalidRequest)?;
        }
        if let Some(market) = &self.market {
            market.validate().map_err(ScanError::InvalidRequest)?;
        }
        Ok(())
    }
}

/// Information the caller knows about the source before scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputInfo {
    pub total_bytes: Option<u64>,
    /// The source itself was deliberately cut short before its real end.
    pub truncated: bool,
    /// This is a bounded head used for a fast preview, not the full source.
    pub preview_only: bool,
}

impl InputInfo {
    pub fn known(total_bytes: u64) -> Self {
        Self {
            total_bytes: Some(total_bytes),
            truncated: false,
            preview_only: false,
        }
    }

    pub fn unknown() -> Self {
        Self {
            total_bytes: None,
            truncated: false,
            preview_only: false,
        }
    }

    pub fn preview(total_bytes: Option<u64>) -> Self {
        Self {
            total_bytes,
            truncated: false,
            preview_only: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    RecordLimit,
    ByteLimit,
    TimeLimit,
    SourceTruncated,
    PreviewOnly,
    Cancelled,
    ParseError,
}

impl IncompleteReason {
    pub fn message(self) -> &'static str {
        match self {
            IncompleteReason::RecordLimit => "record limit reached",
            IncompleteReason::ByteLimit => "byte limit reached",
            IncompleteReason::TimeLimit => "time limit reached",
            IncompleteReason::SourceTruncated => "source was truncated before its end",
            IncompleteReason::PreviewOnly => "preview uses only an initial part of the source",
            IncompleteReason::Cancelled => "scan was cancelled",
            IncompleteReason::ParseError => "parser stopped at a malformed record",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanCoverage {
    /// Data rows scanned, excluding a detected or supplied header.
    pub data_records: u64,
    /// Rows read including the header row.
    pub rows: u64,
    /// Source bytes read. May include a small read-ahead (at most one decoder
    /// chunk) at a stop boundary.
    pub bytes: u64,
    pub total_bytes: Option<u64>,
    pub source_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderReport {
    pub present: bool,
    pub source: ChoiceSource,
    pub raw: Vec<String>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeCounts {
    pub empty: u64,
    pub integer: u64,
    pub float: u64,
    pub boolean: u64,
    pub date: u64,
    pub text: u64,
}

impl TypeCounts {
    pub fn add(&mut self, kind: CellKind) {
        match kind {
            CellKind::Empty => self.empty += 1,
            CellKind::Integer => self.integer += 1,
            CellKind::Float => self.float += 1,
            CellKind::Boolean => self.boolean += 1,
            CellKind::Date => self.date += 1,
            CellKind::Text => self.text += 1,
        }
    }

    pub fn non_empty(&self) -> u64 {
        self.integer + self.float + self.boolean + self.date + self.text
    }

    pub fn inferred(&self) -> CellKind {
        let non_empty = self.non_empty();
        if non_empty == 0 {
            return CellKind::Empty;
        }
        if self.integer + self.float == non_empty {
            return if self.float > 0 {
                CellKind::Float
            } else {
                CellKind::Integer
            };
        }
        if self.boolean == non_empty {
            return CellKind::Boolean;
        }
        if self.date == non_empty {
            return CellKind::Date;
        }
        if self.text == non_empty {
            return CellKind::Text;
        }
        CellKind::Text
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnProfile {
    pub index: usize,
    pub name: String,
    pub inferred: CellKind,
    pub counts: TypeCounts,
    pub max_chars: usize,
    /// A few bounded source values for the UI. Never used for conversion.
    pub samples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewRow {
    /// 1-based data record number. Headers are not counted.
    pub record: u64,
    /// 1-based source line where the record starts.
    pub line: u64,
    pub cells: Vec<String>,
    pub cells_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateRangeStatus {
    Computed,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateRangeReport {
    pub column: usize,
    pub column_name: Option<String>,
    pub format: DateFormat,
    pub status: DateRangeStatus,
    pub min: Option<String>,
    pub max: Option<String>,
    pub parsed: u64,
    pub invalid: u64,
    pub missing: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanReport {
    pub complete: bool,
    pub incomplete_reason: Option<IncompleteReason>,
    /// Only `Some` when `complete` is true. Never present a partial total.
    pub total_records: Option<u64>,
    pub scanned: ScanCoverage,
    pub encoding: EncodingReport,
    pub dialect: DialectReport,
    pub header: HeaderReport,
    pub columns: Vec<ColumnProfile>,
    pub preview_rows: Vec<PreviewRow>,
    pub warnings: Vec<Warning>,
    pub date_ranges: Vec<DateRangeReport>,
    pub market: Option<MarketReport>,
    pub parse_error: Option<String>,
}

impl ScanReport {
    /// Check the coverage invariant. Used by tests and by the worker before
    /// sending a result.
    pub fn validate_invariants(&self) -> Result<(), String> {
        if self.complete {
            if self.total_records != Some(self.scanned.data_records) {
                return Err("complete scan must report an exact total".to_string());
            }
            if self.incomplete_reason.is_some() {
                return Err("complete scan must not carry an incomplete reason".to_string());
            }
        } else {
            if self.total_records.is_some() {
                return Err("incomplete scan must not report an exact total".to_string());
            }
            if self.incomplete_reason.is_none() {
                return Err("incomplete scan must carry a reason".to_string());
            }
        }
        if self.scanned.source_truncated && self.complete {
            return Err("a truncated source can never produce a complete scan".to_string());
        }
        Ok(())
    }

    /// Human coverage line that never claims an exact total for a partial scan.
    pub fn coverage_summary(&self) -> String {
        let bytes = self.scanned.bytes;
        let byte_part = match self.scanned.total_bytes {
            Some(total) if total > 0 => {
                let percent = ((bytes as f64 / total as f64) * 100.0).min(100.0);
                format!("{} of {} bytes ({:.1}%)", bytes, total, percent)
            }
            Some(total) => format!("{} of {} bytes", bytes, total),
            None => format!("{} bytes read", bytes),
        };
        if self.complete {
            format!(
                "complete: {} records scanned, {}",
                self.scanned.data_records, byte_part
            )
        } else {
            let reason = self
                .incomplete_reason
                .map(IncompleteReason::message)
                .unwrap_or("incomplete");
            format!(
                "incomplete ({}): {} records scanned, {}",
                reason, self.scanned.data_records, byte_part
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanProgress {
    pub data_records: u64,
    pub bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    Io(String),
    InvalidRequest(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io(message) => write!(formatter, "I/O error: {message}"),
            ScanError::InvalidRequest(message) => write!(formatter, "invalid request: {message}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<std::io::Error> for ScanError {
    fn from(error: std::io::Error) -> Self {
        ScanError::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{ChoiceSource, EncodingReport};

    fn sample_report(complete: bool) -> ScanReport {
        ScanReport {
            complete,
            incomplete_reason: if complete {
                None
            } else {
                Some(IncompleteReason::RecordLimit)
            },
            total_records: if complete { Some(2) } else { None },
            scanned: ScanCoverage {
                data_records: 2,
                rows: 3,
                bytes: 10,
                total_bytes: Some(20),
                source_truncated: false,
            },
            encoding: EncodingReport {
                label: "utf-8".into(),
                source: ChoiceSource::Auto,
                bom: false,
                replacement_errors: false,
                replacement_chunks: 0,
            },
            dialect: DialectReport {
                delimiter: ',',
                quote: '"',
                source: ChoiceSource::Auto,
                confidence: 100,
            },
            header: HeaderReport {
                present: true,
                source: ChoiceSource::Auto,
                raw: vec!["a".into()],
                columns: vec!["a".into()],
            },
            columns: Vec::new(),
            preview_rows: Vec::new(),
            warnings: Vec::new(),
            date_ranges: Vec::new(),
            market: None,
            parse_error: None,
        }
    }

    #[test]
    fn coverage_invariant_is_enforced() {
        assert!(sample_report(true).validate_invariants().is_ok());
        assert!(sample_report(false).validate_invariants().is_ok());
        let mut bad = sample_report(false);
        bad.total_records = Some(2);
        assert!(bad.validate_invariants().is_err());
    }

    #[test]
    fn partial_summary_never_says_total() {
        let report = sample_report(false);
        let summary = report.coverage_summary();
        assert!(summary.starts_with("incomplete"));
        assert!(summary.contains("2 records"));
    }
}
