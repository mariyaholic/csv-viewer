// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use csvview_core::{scan_file, ScanRequest, WarningCode};

#[test]
fn golden_fixture_reports_reviewable_changes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden.csv");
    let report = scan_file(&path, &ScanRequest::full()).unwrap();
    assert!(report.complete);
    let codes: Vec<WarningCode> = report.warnings.iter().map(|warning| warning.code).collect();
    for code in [
        WarningCode::LeadingZeros,
        WarningCode::AmbiguousDate,
        WarningCode::BooleanSpellings,
        WarningCode::LongNumber,
        WarningCode::FormulaLikeText,
        WarningCode::LinkLikeText,
        WarningCode::QuotedMultiline,
    ] {
        assert!(codes.contains(&code), "missing {code:?} in {codes:?}");
    }

    let leading = report
        .warnings
        .iter()
        .find(|warning| warning.code == WarningCode::LeadingZeros)
        .unwrap();
    assert_eq!(leading.occurrences, 2);
    assert_eq!(leading.column_name.as_deref(), Some("id"));

    let formula = report
        .warnings
        .iter()
        .find(|warning| warning.code == WarningCode::FormulaLikeText)
        .unwrap();
    assert_eq!(formula.occurrences, 1);
    assert!(formula.samples[0].value.contains("SUM"));
}

#[test]
fn warning_samples_never_include_control_characters() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden.csv");
    let report = scan_file(&path, &ScanRequest::full()).unwrap();
    for warning in &report.warnings {
        for sample in &warning.samples {
            assert!(!sample.value.chars().any(|ch| ch == '\u{1b}'));
            assert!(!sample.value.contains('\n'));
        }
    }
}
