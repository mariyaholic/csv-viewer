// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use csvview_core::{scan_file, ScanRequest};

/// A fixture report is compared as a whole, not just field by field. Regenerate
/// intentionally with `UPDATE_GOLDEN=1 cargo test -p csvview-core --test golden`.
#[test]
fn parser_report_matches_the_reviewed_golden_file() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let input = base.join("fixtures").join("golden.csv");
    let golden_path = base.join("golden").join("golden_report.json");
    let report = scan_file(&input, &ScanRequest::full()).unwrap();
    report.validate_invariants().unwrap();
    let actual = serde_json::to_value(&report).unwrap();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        std::fs::write(
            &golden_path,
            serde_json::to_string_pretty(&actual).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    assert_eq!(
        actual, expected,
        "golden report changed; review the source text and regenerate deliberately"
    );
}

#[test]
fn rendered_preview_is_stable_and_never_claims_a_total_when_partial() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let input = base.join("fixtures").join("golden.csv");
    let report = scan_file(&input, &ScanRequest::full()).unwrap();
    let rendered = csvview_core::text::render_text(&report, 100, 10);
    assert!(rendered.contains("Total records: 3"));
    assert!(rendered.contains("Made by Maria"));
    assert!(rendered.contains("FormulaLikeText"));
    assert!(!rendered.contains("Incomplete scan"));

    let mut preview = ScanRequest::preview();
    preview.limits.max_records = Some(1);
    let report = scan_file(&input, &preview).unwrap();
    let rendered = csvview_core::text::render_text(&report, 100, 10);
    assert!(!rendered.contains("Total records:"));
    assert!(rendered.contains("Incomplete"));
}
