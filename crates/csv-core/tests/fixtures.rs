// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use csvview_core::{
    scan_file, DateFormat, DateRangeRequest, DateRangeStatus, Dialect, DialectChoice,
    EncodingChoice, EncodingKind, HeaderChoice, IncompleteReason, ScanRequest, WarningCode,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn scan(name: &str, request: &ScanRequest) -> csvview_core::ScanReport {
    scan_file(&fixture(name), request).unwrap()
}

#[test]
fn detects_utf8_bom_and_utf16_fixtures() {
    let report = scan("utf8_bom.csv", &ScanRequest::full());
    assert_eq!(report.encoding.label, "utf-8");
    assert!(report.encoding.bom);
    assert_eq!(report.columns[0].name, "name");

    let report = scan("utf16le_bom.csv", &ScanRequest::full());
    assert_eq!(report.encoding.label, "utf-16le");
    assert!(report.encoding.bom);
    assert_eq!(report.total_records, Some(1));

    let report = scan("utf16be_bom.csv", &ScanRequest::full());
    assert_eq!(report.encoding.label, "utf-16be");
    assert!(report.encoding.bom);
    assert_eq!(report.total_records, Some(1));
}

#[test]
fn decodes_windows_1252_without_replacing_text() {
    let mut request = ScanRequest::full();
    request.header = HeaderChoice::Present;
    let report = scan("windows1252.csv", &request);
    assert_eq!(report.encoding.label, "windows-1252");
    assert!(!report.encoding.replacement_errors);
    assert_eq!(report.preview_rows[0].cells[0], "André");
    assert_eq!(report.preview_rows[0].cells[1], "Zürich");
}

#[test]
fn detects_semicolon_tab_and_pipe() {
    assert_eq!(
        scan("semicolon.csv", &ScanRequest::full())
            .dialect
            .delimiter,
        ';'
    );
    assert_eq!(
        scan("tab.tsv", &ScanRequest::full()).dialect.delimiter,
        '\t'
    );
    assert_eq!(
        scan("pipe.csv", &ScanRequest::full()).dialect.delimiter,
        '|'
    );
}

#[test]
fn preserves_quoted_multiline_cells() {
    let report = scan("quoted_multiline.csv", &ScanRequest::full());
    assert_eq!(report.total_records, Some(2));
    assert_eq!(report.preview_rows[0].cells[1], "first\nsecond");
    assert_eq!(report.preview_rows[1].cells[1], "third, with comma");
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == WarningCode::QuotedMultiline));
}

#[test]
fn ragged_rows_extend_columns_and_warn() {
    let report = scan("ragged.csv", &ScanRequest::full());
    assert_eq!(report.columns.len(), 4);
    assert_eq!(report.columns[3].name, "column_4");
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == WarningCode::RaggedRow));
}

#[test]
fn empty_file_is_a_complete_zero_row_scan() {
    let report = scan("empty.csv", &ScanRequest::full());
    assert!(report.complete);
    assert_eq!(report.total_records, Some(0));
    assert!(report.columns.is_empty());
    assert!(!report.header.present);
}

#[test]
fn single_column_file_has_no_invented_header() {
    let report = scan("single_column.csv", &ScanRequest::full());
    assert_eq!(report.columns.len(), 1);
    assert!(!report.header.present);
    assert_eq!(report.total_records, Some(4));
}

#[test]
fn crlf_file_is_complete() {
    let report = scan("crlf.csv", &ScanRequest::full());
    assert!(report.complete);
    assert_eq!(report.total_records, Some(1));
}

#[test]
fn malformed_quote_does_not_claim_completeness() {
    let report = scan("unterminated_quote.csv", &ScanRequest::full());
    assert!(!report.complete);
    assert_eq!(report.total_records, None);
    assert!(report.parse_error.is_some() || report.incomplete_reason.is_some());
}

#[test]
fn complete_date_range_uses_the_requested_interpretation() {
    let mut request = ScanRequest::full();
    request.date_ranges.push(DateRangeRequest {
        column: 0,
        format: DateFormat::Ymd,
    });
    let report = scan("dates.csv", &request);
    let range = &report.date_ranges[0];
    assert_eq!(range.status, DateRangeStatus::Computed);
    assert_eq!(range.min.as_deref(), Some("2024-01-01"));
    assert_eq!(range.max.as_deref(), Some("2024-02-29"));
    assert_eq!(range.parsed, 3);
    assert_eq!(range.invalid, 0);
}

#[test]
fn explicit_overrides_win_over_detection() {
    let mut request = ScanRequest::full();
    request.dialect = DialectChoice::Explicit(Dialect {
        delimiter: ';',
        ..Dialect::default()
    });
    request.header = HeaderChoice::Present;
    let report = scan("semicolon.csv", &request);
    assert_eq!(report.columns.len(), 2);
    assert_eq!(report.columns[1].name, "value");

    let mut request = ScanRequest::full();
    request.encoding = EncodingChoice::Explicit(EncodingKind::Utf8);
    request.header = HeaderChoice::Present;
    let report = scan("windows1252.csv", &request);
    assert!(report.encoding.replacement_errors);
    assert!(report.preview_rows[0].cells[0].contains('\u{fffd}'));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == WarningCode::ReplacementErrors));
}

#[test]
fn huge_field_is_truncated_with_a_warning() {
    // Long relative to the 1024-character display cap but still inside the
    // per-field byte budget (`max_field_chars * 4`), so the record finishes
    // and only the display truncates.
    let big = "x".repeat(3_000);
    let text = format!("a,b\n1,{big}\n");
    let mut request = ScanRequest::full();
    request.header = HeaderChoice::Present;
    request.limits.max_field_chars = 1024;
    let report = scan_text(&text, &request);
    assert!(report.complete);
    assert_eq!(report.preview_rows[0].cells[1].chars().count(), 1024);
    assert!(report.preview_rows[0].cells_truncated);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == WarningCode::LongField));
}

#[test]
fn field_beyond_the_byte_budget_stops_the_scan_as_partial() {
    // A single field past `max_field_chars * 4` ends the scan early with a
    // parse error: memory must stay bounded inside one record, not only
    // between records.
    let big = "x".repeat(200_000);
    let text = format!("a,b\n1,{big}\n");
    let mut request = ScanRequest::full();
    request.header = HeaderChoice::Present;
    request.limits.max_field_chars = 1024;
    let report = scan_text(&text, &request);
    assert!(!report.complete);
    assert_eq!(report.incomplete_reason, Some(IncompleteReason::ParseError));
    assert!(report
        .parse_error
        .as_deref()
        .is_some_and(|error| error.contains("per-field byte budget")));
    // No data record ever finished: the first one is the oversized field.
    assert_eq!(report.scanned.data_records, 0);
}

fn scan_text(text: &str, request: &ScanRequest) -> csvview_core::ScanReport {
    csvview_core::scan(
        std::io::Cursor::new(text.as_bytes().to_vec()),
        csvview_core::InputInfo::known(text.len() as u64),
        request,
    )
    .unwrap()
}

#[test]
fn explicit_header_absent_keeps_all_rows_as_data() {
    let mut request = ScanRequest::full();
    request.header = HeaderChoice::Absent;
    let report = scan("dates.csv", &request);
    assert_eq!(report.total_records, Some(4));
    assert_eq!(report.preview_rows[0].cells[0], "date");
}
