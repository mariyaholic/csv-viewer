// SPDX-License-Identifier: Apache-2.0

//! Core library for the CSV viewer and preview.
//!
//! Made by Maria. Free and open source under Apache-2.0.
//!
//! The core is platform-independent: it detects encodings and dialects, scans
//! CSV without converting source cells, records review warnings, computes
//! explicitly requested date ranges, and offers optional market-data
//! heuristics. Windows COM code lives in the `csv-preview` crate behind
//! `cfg(windows)`.

pub mod dialect;
pub mod encoding;
pub mod market;
pub mod model;
pub mod scan;
pub mod settings;
pub mod table;
pub mod text;
pub mod types;
pub mod warnings;

pub use dialect::{Dialect, DialectChoice, DialectReport};
pub use encoding::{ChoiceSource, EncodingKind, EncodingReport};
pub use market::{
    BaselineComparison, ColumnRef, MarketBaseline, MarketFields, MarketFinding,
    MarketFindingCode, MarketMetrics, MarketReport, MarketRequest, MarketSeverity,
    MarketThresholds,
};
pub use model::{
    ColumnProfile, DateRangeReport, DateRangeRequest, DateRangeStatus, EncodingChoice, HeaderChoice,
    HeaderReport, IncompleteReason, InputInfo, PreviewRow, ScanCoverage, ScanError, ScanLimits,
    ScanProgress, ScanReport, ScanRequest,
};
pub use scan::{
    scan, scan_file, scan_file_with_info, scan_file_with_progress, scan_with_progress,
};
pub use settings::Settings;
pub use table::TableModel;
pub use types::{CellKind, CivilDate, DateFormat};
pub use warnings::{Severity, Warning, WarningCode};
