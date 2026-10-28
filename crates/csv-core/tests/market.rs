// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use csvview_core::{
    scan_file, BaselineComparison, ColumnRef, MarketBaseline, MarketFields, MarketFindingCode,
    MarketRequest, MarketThresholds, ScanRequest,
};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn market_request() -> MarketRequest {
    MarketRequest {
        fields: MarketFields {
            open: ColumnRef::Name("open".into()),
            high: ColumnRef::Name("high".into()),
            low: ColumnRef::Name("low".into()),
            close: ColumnRef::Name("close".into()),
            spread: None,
            volume: Some(ColumnRef::Name("volume".into())),
        },
        thresholds: MarketThresholds::default(),
        baseline: None,
        min_sample: 3,
        instrument: Some("TEST".into()),
    }
}

#[test]
fn clean_fixture_has_no_spread_or_open_flags() {
    let mut request = ScanRequest::full();
    request.market = Some(market_request());
    let report = scan_file(&fixture("market.csv"), &request).unwrap();
    let market = report.market.unwrap();
    assert!(market.available);
    assert_eq!(market.sample_rows, 3);
    assert_eq!(market.missing_rows, 0);
    assert!(!market
        .findings
        .iter()
        .any(|finding| finding.code == MarketFindingCode::ConstantSpread));
    assert!(!market
        .findings
        .iter()
        .any(|finding| finding.code == MarketFindingCode::OpenEqualsPreviousClose));
}

#[test]
fn constant_spread_fixture_is_flagged_with_thresholds() {
    let mut request = ScanRequest::full();
    let mut market = market_request();
    market.fields.volume = None;
    request.market = Some(market);
    let report = scan_file(&fixture("market_constant_spread.csv"), &request).unwrap();
    let market = report.market.unwrap();
    let finding = market
        .findings
        .iter()
        .find(|finding| finding.code == MarketFindingCode::ConstantSpread)
        .expect("constant spread finding");
    assert_eq!(finding.sample_size, 3);
    assert_eq!(finding.count, 3);
    assert!(finding.threshold >= 0.0);
    assert!(market.thresholds.constant_spread_tolerance >= 0.0);
}

#[test]
fn baseline_without_current_instrument_is_unavailable() {
    let mut request = ScanRequest::full();
    let mut market = market_request();
    market.baseline = Some(MarketBaseline {
        instrument: "TEST".into(),
        as_of: "2024-01-01".into(),
        licensed: true,
        median_close: 100.0,
        source: "fixture".into(),
    });
    market.instrument = None;
    request.market = Some(market);
    let report = scan_file(&fixture("market.csv"), &request).unwrap();
    let market = report.market.unwrap();
    assert!(matches!(
        market.metrics.baseline,
        BaselineComparison::Unavailable { .. }
    ));
}

#[test]
fn baseline_with_matching_dated_instrument_is_available() {
    let mut request = ScanRequest::full();
    let mut market = market_request();
    market.baseline = Some(MarketBaseline {
        instrument: "TEST".into(),
        as_of: "2024-01-01".into(),
        licensed: true,
        median_close: 100.0,
        source: "fixture".into(),
    });
    request.market = Some(market);
    let report = scan_file(&fixture("market.csv"), &request).unwrap();
    let market = report.market.unwrap();
    match market.metrics.baseline {
        BaselineComparison::Available {
            as_of, sample_size, ..
        } => {
            assert_eq!(as_of, "2024-01-01");
            assert_eq!(sample_size, 3);
        }
        other => panic!("expected available baseline, got {other:?}"),
    }
}

#[test]
fn missing_mapped_column_is_unavailable_not_an_error() {
    let mut request = ScanRequest::full();
    let mut market = market_request();
    market.fields.close = ColumnRef::Name("not_here".into());
    request.market = Some(market);
    let report = scan_file(&fixture("market.csv"), &request).unwrap();
    let market = report.market.unwrap();
    assert!(!market.available);
    assert!(market.unavailable_reason.unwrap().contains("not_here"));
}
