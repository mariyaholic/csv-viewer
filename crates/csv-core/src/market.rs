// SPDX-License-Identifier: Apache-2.0

//! Optional market-data heuristics.
//!
//! These checks describe patterns in a user-mapped file. They are not proof of
//! fraud or authenticity and are not financial advice. Every finding reports
//! its threshold, sample size and missing data so it can be reviewed rather
//! than trusted blindly. Baseline comparison is unavailable unless a dated,
//! licensed, instrument-specific local baseline is supplied.

use serde::{Deserialize, Serialize};

use crate::types::{parse_date, DateFormat};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum ColumnRef {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketFields {
    pub open: ColumnRef,
    pub high: ColumnRef,
    pub low: ColumnRef,
    pub close: ColumnRef,
    pub spread: Option<ColumnRef>,
    pub volume: Option<ColumnRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketThresholds {
    pub constant_spread_tolerance: f64,
    /// Relative difference at which open is considered equal to previous close.
    pub price_equality_tolerance: f64,
    /// Absolute close-to-close log return that is always an outlier.
    pub return_outlier_abs: f64,
    /// Standard deviations from the mean return that mark an outlier.
    pub return_outlier_sigma: f64,
    /// A close-to-close ratio above this (or below its reciprocal) is a jump.
    pub price_jump_ratio: f64,
    pub baseline_tolerance: f64,
    pub max_return_sample: usize,
}

impl Default for MarketThresholds {
    fn default() -> Self {
        Self {
            constant_spread_tolerance: 1e-6,
            price_equality_tolerance: 1e-9,
            return_outlier_abs: 0.20,
            return_outlier_sigma: 6.0,
            price_jump_ratio: 10.0,
            baseline_tolerance: 0.10,
            max_return_sample: 100_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketBaseline {
    pub instrument: String,
    /// ISO date (`YYYY-MM-DD`). A baseline without a date is unusable.
    pub as_of: String,
    /// The user must confirm the local baseline is licensed for this use.
    pub licensed: bool,
    pub median_close: f64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketRequest {
    pub fields: MarketFields,
    pub thresholds: MarketThresholds,
    pub baseline: Option<MarketBaseline>,
    pub min_sample: usize,
    /// Instrument identifier for the current file. Required for an available
    /// baseline comparison; otherwise a baseline is reported as unavailable.
    pub instrument: Option<String>,
}

impl MarketRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.min_sample == 0 {
            return Err("market min_sample must be greater than zero".to_string());
        }
        let t = &self.thresholds;
        for (name, value) in [
            ("constant_spread_tolerance", t.constant_spread_tolerance),
            ("price_equality_tolerance", t.price_equality_tolerance),
            ("return_outlier_abs", t.return_outlier_abs),
            ("return_outlier_sigma", t.return_outlier_sigma),
            ("baseline_tolerance", t.baseline_tolerance),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("market threshold {name} must be finite and non-negative"));
            }
        }
        if !t.price_jump_ratio.is_finite() || t.price_jump_ratio <= 1.0 {
            return Err("market price_jump_ratio must be greater than one".to_string());
        }
        if t.max_return_sample == 0 {
            return Err("market max_return_sample must be greater than zero".to_string());
        }
        if let Some(baseline) = &self.baseline {
            if !baseline.median_close.is_finite() || baseline.median_close <= 0.0 {
                return Err("baseline median_close must be positive and finite".to_string());
            }
            if parse_date(&baseline.as_of, DateFormat::Ymd).is_none() {
                return Err("baseline as_of must be an ISO date".to_string());
            }
            if baseline.instrument.trim().is_empty() {
                return Err("baseline instrument must not be empty".to_string());
            }
        }
        if let Some(instrument) = &self.instrument {
            if instrument.trim().is_empty() {
                return Err("market instrument must not be empty when supplied".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketFindingCode {
    ConstantSpread,
    OpenEqualsPreviousClose,
    ReturnTail,
    PricePlausibility,
    MissingData,
    BaselineDifference,
    SmallSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketFinding {
    pub code: MarketFindingCode,
    pub severity: MarketSeverity,
    pub message: String,
    pub count: u64,
    pub sample_size: u64,
    pub threshold: f64,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum BaselineComparison {
    Available {
        as_of: String,
        source: String,
        baseline_median_close: f64,
        current_median_close: f64,
        relative_difference: f64,
        within_tolerance: bool,
        sample_size: u64,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub valid_rows: u64,
    pub missing_rows: u64,
    pub spread_rows: u64,
    pub spread_min: Option<f64>,
    pub spread_max: Option<f64>,
    pub open_previous_pairs: u64,
    pub open_equals_previous_close: u64,
    pub return_sample_size: u64,
    pub return_sample_truncated: bool,
    pub return_mean: Option<f64>,
    pub return_stddev: Option<f64>,
    pub return_max_abs: Option<f64>,
    pub return_outliers: u64,
    pub price_jumps: u64,
    pub non_positive_price_rows: u64,
    pub invalid_ohlc_rows: u64,
    pub close_outside_range_rows: u64,
    pub open_outside_range_rows: u64,
    pub baseline: BaselineComparison,
}

impl Default for BaselineComparison {
    fn default() -> Self {
        BaselineComparison::Unavailable {
            reason: "no dated, licensed, instrument-specific baseline provided".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketReport {
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub instrument: Option<String>,
    pub sample_rows: u64,
    pub missing_rows: u64,
    pub coverage_incomplete: bool,
    pub thresholds: MarketThresholds,
    pub metrics: MarketMetrics,
    pub findings: Vec<MarketFinding>,
    pub disclaimer: String,
}

pub const MARKET_DISCLAIMER: &str =
    "Heuristics only; not proof of fraud or authenticity and not financial advice.";

struct ResolvedFields {
    open: usize,
    high: usize,
    low: usize,
    close: usize,
    spread: Option<usize>,
    volume: Option<usize>,
}

pub struct MarketAccumulator {
    request: MarketRequest,
    resolved: Option<ResolvedFields>,
    unavailable_reason: Option<String>,
    sample_rows: u64,
    missing_rows: u64,
    spread_values: Vec<f64>,
    prev_close: Option<f64>,
    open_previous_pairs: u64,
    open_equals: u64,
    returns: Vec<f64>,
    return_sample_truncated: bool,
    closes: Vec<f64>,
    price_jumps: u64,
    non_positive: u64,
    invalid_ohlc: u64,
    close_outside: u64,
    open_outside: u64,
}

impl MarketAccumulator {
    pub fn new(request: MarketRequest, columns: &[String]) -> Self {
        let (resolved, unavailable_reason) = match resolve_fields(&request.fields, columns) {
            Ok(fields) => (Some(fields), None),
            Err(reason) => (None, Some(reason)),
        };
        Self {
            request,
            resolved,
            unavailable_reason,
            sample_rows: 0,
            missing_rows: 0,
            spread_values: Vec::new(),
            prev_close: None,
            open_previous_pairs: 0,
            open_equals: 0,
            returns: Vec::new(),
            return_sample_truncated: false,
            closes: Vec::new(),
            price_jumps: 0,
            non_positive: 0,
            invalid_ohlc: 0,
            close_outside: 0,
            open_outside: 0,
        }
    }

    pub fn feed(&mut self, row: &[String]) {
        let Some(fields) = &self.resolved else {
            return;
        };
        let open = parse_price(row.get(fields.open));
        let high = parse_price(row.get(fields.high));
        let low = parse_price(row.get(fields.low));
        let close = parse_price(row.get(fields.close));
        let (Some(open), Some(high), Some(low), Some(close)) = (open, high, low, close) else {
            self.missing_rows += 1;
            return;
        };
        if fields.spread.is_some() && parse_price(fields.spread.and_then(|i| row.get(i))).is_none() {
            self.missing_rows += 1;
        }
        if fields.volume.is_some() && parse_price(fields.volume.and_then(|i| row.get(i))).is_none() {
            self.missing_rows += 1;
        }
        self.sample_rows += 1;

        let max_return_sample = self.request.thresholds.max_return_sample;
        if self.spread_values.len() < max_return_sample {
            self.spread_values.push(high - low);
        }
        if self.closes.len() < max_return_sample {
            self.closes.push(close);
        }

        if open <= 0.0 || high <= 0.0 || low <= 0.0 || close <= 0.0 {
            self.non_positive += 1;
        }
        if high < low {
            self.invalid_ohlc += 1;
        }
        if close < low || close > high {
            self.close_outside += 1;
        }
        if open < low || open > high {
            self.open_outside += 1;
        }

        if let Some(previous) = self.prev_close {
            self.open_previous_pairs += 1;
            if previous.abs() > f64::EPSILON {
                let relative = (open - previous).abs() / previous.abs();
                if relative <= self.request.thresholds.price_equality_tolerance {
                    self.open_equals += 1;
                }
                let ratio = close / previous;
                if ratio > self.request.thresholds.price_jump_ratio
                    || ratio < 1.0 / self.request.thresholds.price_jump_ratio
                {
                    self.price_jumps += 1;
                }
            }
            if previous > 0.0 && close > 0.0 {
                if self.returns.len() < max_return_sample {
                    self.returns.push((close / previous).ln());
                } else {
                    self.return_sample_truncated = true;
                }
            }
        }
        self.prev_close = Some(close);
    }

    pub fn finish(self, scan_complete: bool) -> MarketReport {
        let thresholds = self.request.thresholds.clone();
        if self.resolved.is_none() {
            return MarketReport {
                available: false,
                unavailable_reason: self.unavailable_reason,
                instrument: self.request.baseline.as_ref().map(|b| b.instrument.clone()),
                sample_rows: 0,
                missing_rows: 0,
                coverage_incomplete: !scan_complete,
                thresholds,
                metrics: MarketMetrics::default(),
                findings: Vec::new(),
                disclaimer: MARKET_DISCLAIMER.to_string(),
            };
        }

        let spread_min = self
            .spread_values
            .iter()
            .copied()
            .reduce(f64::min);
        let spread_max = self
            .spread_values
            .iter()
            .copied()
            .reduce(f64::max);
        let return_sample_size = self.returns.len() as u64;
        let (return_mean, return_stddev) = mean_stddev(&self.returns);
        let return_max_abs = self
            .returns
            .iter()
            .map(|value| value.abs())
            .reduce(f64::max);
        let return_outliers = match (return_mean, return_stddev) {
            (Some(mean), Some(stddev)) => self
                .returns
                .iter()
                .filter(|value| {
                    let z = if stddev > 0.0 {
                        (**value - mean).abs() / stddev
                    } else {
                        0.0
                    };
                    z > thresholds.return_outlier_sigma
                        || (value.abs() > thresholds.return_outlier_abs)
                })
                .count() as u64,
            _ => 0,
        };

        let mut findings = Vec::new();
        if self.sample_rows < self.request.min_sample as u64 {
            findings.push(MarketFinding {
                code: MarketFindingCode::SmallSample,
                severity: MarketSeverity::Info,
                message: "Too few valid rows for reliable market heuristics.".to_string(),
                count: self.sample_rows,
                sample_size: self.sample_rows,
                threshold: self.request.min_sample as f64,
                value: None,
            });
        }

        if let (Some(min), Some(max)) = (spread_min, spread_max) {
            if self.sample_rows >= self.request.min_sample as u64
                && (max - min).abs() <= thresholds.constant_spread_tolerance
            {
                findings.push(MarketFinding {
                    code: MarketFindingCode::ConstantSpread,
                    severity: MarketSeverity::Warning,
                    message: "High minus low is constant within tolerance.".to_string(),
                    count: self.sample_rows,
                    sample_size: self.sample_rows,
                    threshold: thresholds.constant_spread_tolerance,
                    value: Some((min + max) / 2.0),
                });
            }
        }

        if self.open_previous_pairs >= self.request.min_sample as u64 {
            let ratio = self.open_equals as f64 / self.open_previous_pairs as f64;
            if ratio >= 0.9 {
                findings.push(MarketFinding {
                    code: MarketFindingCode::OpenEqualsPreviousClose,
                    severity: MarketSeverity::Warning,
                    message: "Open equals the previous close for most rows.".to_string(),
                    count: self.open_equals,
                    sample_size: self.open_previous_pairs,
                    threshold: 0.9,
                    value: Some(ratio),
                });
            }
        }

        if return_outliers > 0 {
            findings.push(MarketFinding {
                code: MarketFindingCode::ReturnTail,
                severity: MarketSeverity::Warning,
                message: "Close-to-close returns contain tail values beyond the threshold."
                    .to_string(),
                count: return_outliers,
                sample_size: return_sample_size,
                threshold: thresholds.return_outlier_abs,
                value: return_max_abs,
            });
        }

        let implausible = self.non_positive
            + self.invalid_ohlc
            + self.close_outside
            + self.open_outside
            + self.price_jumps;
        if implausible > 0 {
            findings.push(MarketFinding {
                code: MarketFindingCode::PricePlausibility,
                severity: MarketSeverity::Warning,
                message: "Prices include non-positive values, inconsistent OHLC ranges or jumps."
                    .to_string(),
                count: implausible,
                sample_size: self.sample_rows,
                threshold: thresholds.price_jump_ratio,
                value: None,
            });
        }
        if self.missing_rows > 0 {
            findings.push(MarketFinding {
                code: MarketFindingCode::MissingData,
                severity: MarketSeverity::Info,
                message: "Some rows are missing a mapped price field or contain an unreadable value."
                    .to_string(),
                count: self.missing_rows,
                sample_size: self.sample_rows + self.missing_rows,
                threshold: 0.0,
                value: None,
            });
        }

        let baseline = self.compare_baseline();
        if let BaselineComparison::Available {
            relative_difference,
            within_tolerance,
            sample_size,
            ..
        } = &baseline
        {
            if !*within_tolerance {
                findings.push(MarketFinding {
                    code: MarketFindingCode::BaselineDifference,
                    severity: MarketSeverity::Warning,
                    message: "Current median close differs from the dated baseline beyond tolerance."
                        .to_string(),
                    count: *sample_size,
                    sample_size: *sample_size,
                    threshold: thresholds.baseline_tolerance,
                    value: Some(*relative_difference),
                });
            }
        }

        MarketReport {
            available: true,
            unavailable_reason: None,
            instrument: self.request.baseline.as_ref().map(|b| b.instrument.clone()),
            sample_rows: self.sample_rows,
            missing_rows: self.missing_rows,
            coverage_incomplete: !scan_complete,
            thresholds,
            metrics: MarketMetrics {
                valid_rows: self.sample_rows,
                missing_rows: self.missing_rows,
                spread_rows: self.spread_values.len() as u64,
                spread_min,
                spread_max,
                open_previous_pairs: self.open_previous_pairs,
                open_equals_previous_close: self.open_equals,
                return_sample_size,
                return_sample_truncated: self.return_sample_truncated,
                return_mean,
                return_stddev,
                return_max_abs,
                return_outliers,
                price_jumps: self.price_jumps,
                non_positive_price_rows: self.non_positive,
                invalid_ohlc_rows: self.invalid_ohlc,
                close_outside_range_rows: self.close_outside,
                open_outside_range_rows: self.open_outside,
                baseline,
            },
            findings,
            disclaimer: MARKET_DISCLAIMER.to_string(),
        }
    }

    fn compare_baseline(&self) -> BaselineComparison {
        let Some(baseline) = &self.request.baseline else {
            return BaselineComparison::Unavailable {
                reason: "no dated, licensed, instrument-specific baseline provided".to_string(),
            };
        };
        if !baseline.licensed {
            return BaselineComparison::Unavailable {
                reason: "baseline is not confirmed as licensed for local comparison".to_string(),
            };
        }
        if parse_date(&baseline.as_of, DateFormat::Ymd).is_none() {
            return BaselineComparison::Unavailable {
                reason: "baseline date is missing or invalid".to_string(),
            };
        }
        let Some(current_instrument) = &self.request.instrument else {
            return BaselineComparison::Unavailable {
                reason: "current instrument is not specified, so a baseline cannot be matched"
                    .to_string(),
            };
        };
        if !current_instrument
            .trim()
            .eq_ignore_ascii_case(baseline.instrument.trim())
        {
            return BaselineComparison::Unavailable {
                reason: format!(
                    "baseline instrument {:?} does not match current instrument {:?}",
                    baseline.instrument, current_instrument
                ),
            };
        }
        let Some(current) = median(&self.closes) else {
            return BaselineComparison::Unavailable {
                reason: "no valid current closes for comparison".to_string(),
            };
        };
        let relative_difference = (current - baseline.median_close).abs() / baseline.median_close;
        BaselineComparison::Available {
            as_of: baseline.as_of.clone(),
            source: baseline.source.clone(),
            baseline_median_close: baseline.median_close,
            current_median_close: current,
            relative_difference,
            within_tolerance: relative_difference <= self.request.thresholds.baseline_tolerance,
            sample_size: self.closes.len() as u64,
        }
    }
}

fn parse_price(value: Option<&String>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let parsed = value.parse::<f64>().ok()?;
    if parsed.is_finite() {
        Some(parsed)
    } else {
        None
    }
}

fn resolve_fields(fields: &MarketFields, columns: &[String]) -> Result<ResolvedFields, String> {
    let resolve = |reference: &ColumnRef| -> Result<usize, String> {
        match reference {
            ColumnRef::Index(index) => {
                if *index < columns.len() {
                    Ok(*index)
                } else {
                    Err(format!("column index {index} is out of range"))
                }
            }
            ColumnRef::Name(name) => columns
                .iter()
                .position(|column| column.trim().eq_ignore_ascii_case(name.trim()))
                .ok_or_else(|| format!("column {name:?} was not found")),
        }
    };
    let open = resolve(&fields.open)?;
    let high = resolve(&fields.high)?;
    let low = resolve(&fields.low)?;
    let close = resolve(&fields.close)?;
    let spread = match &fields.spread {
        Some(reference) => Some(resolve(reference)?),
        None => None,
    };
    let volume = match &fields.volume {
        Some(reference) => Some(resolve(reference)?),
        None => None,
    };
    Ok(ResolvedFields {
        open,
        high,
        low,
        close,
        spread,
        volume,
    })
}

fn mean_stddev(values: &[f64]) -> (Option<f64>, Option<f64>) {
    if values.is_empty() {
        return (None, None);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (Some(mean), None);
    }
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    (Some(mean), Some(variance.sqrt()))
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        Some((sorted[middle - 1] + sorted[middle]) / 2.0)
    } else {
        Some(sorted[middle])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> MarketFields {
        MarketFields {
            open: ColumnRef::Name("open".into()),
            high: ColumnRef::Name("high".into()),
            low: ColumnRef::Name("low".into()),
            close: ColumnRef::Name("close".into()),
            spread: None,
            volume: None,
        }
    }

    fn request() -> MarketRequest {
        MarketRequest {
            fields: fields(),
            thresholds: MarketThresholds::default(),
            baseline: None,
            min_sample: 3,
            instrument: None,
        }
    }

    fn row(open: f64, high: f64, low: f64, close: f64) -> Vec<String> {
        vec![
            open.to_string(),
            high.to_string(),
            low.to_string(),
            close.to_string(),
        ]
    }

    fn columns() -> Vec<String> {
        vec!["open".into(), "high".into(), "low".into(), "close".into()]
    }

    fn run(rows: &[Vec<String>], request: MarketRequest) -> MarketReport {
        let mut accumulator = MarketAccumulator::new(request, &columns());
        for row in rows {
            accumulator.feed(row);
        }
        accumulator.finish(true)
    }

    #[test]
    fn clean_market_has_no_warning_findings() {
        let rows = vec![
            row(100.0, 102.0, 99.0, 101.0),
            row(101.5, 103.0, 100.5, 102.0),
            row(102.5, 104.0, 101.0, 103.0),
            row(103.5, 105.0, 102.5, 104.0),
        ];
        let report = run(&rows, request());
        assert!(report.available);
        assert_eq!(report.sample_rows, 4);
        assert!(!report
            .findings
            .iter()
            .any(|f| f.code == MarketFindingCode::ConstantSpread));
        assert!(!report
            .findings
            .iter()
            .any(|f| f.code == MarketFindingCode::OpenEqualsPreviousClose));
    }

    #[test]
    fn constant_spread_is_flagged() {
        let rows = (0..5)
            .map(|i| row(100.0 + i as f64, 101.0 + i as f64, 100.0 + i as f64, 100.5 + i as f64))
            .collect::<Vec<_>>();
        let report = run(&rows, request());
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == MarketFindingCode::ConstantSpread));
    }

    #[test]
    fn open_equal_previous_close_is_flagged() {
        let rows = vec![
            row(100.0, 102.0, 99.0, 101.0),
            row(101.0, 103.0, 100.0, 102.0),
            row(102.0, 104.0, 101.0, 103.0),
            row(103.0, 105.0, 102.0, 104.0),
        ];
        let report = run(&rows, request());
        let finding = report
            .findings
            .iter()
            .find(|f| f.code == MarketFindingCode::OpenEqualsPreviousClose)
            .unwrap();
        assert_eq!(finding.count, 3);
    }

    #[test]
    fn implausible_prices_are_reported() {
        let rows = vec![
            row(0.0, 10.0, 9.0, 9.5),
            row(10.0, 9.0, 11.0, 9.0),
            row(10.0, 12.0, 9.0, 13.0),
        ];
        let report = run(&rows, request());
        assert!(report
            .findings
            .iter()
            .any(|f| f.code == MarketFindingCode::PricePlausibility));
        assert_eq!(report.metrics.non_positive_price_rows, 1);
        assert_eq!(report.metrics.invalid_ohlc_rows, 1);
        assert_eq!(report.metrics.close_outside_range_rows, 2);
        assert_eq!(report.metrics.open_outside_range_rows, 2);
    }

    #[test]
    fn missing_fields_are_unavailable() {
        let mut request = request();
        request.fields.close = ColumnRef::Name("missing".into());
        let report = run(&[], request);
        assert!(!report.available);
        assert!(report.unavailable_reason.unwrap().contains("missing"));
    }

    #[test]
    fn baseline_requires_license_and_instrument() {
        let mut unlicensed = request();
        unlicensed.baseline = Some(MarketBaseline {
            instrument: "TEST".into(),
            as_of: "2024-01-01".into(),
            licensed: false,
            median_close: 100.0,
            source: "fixture".into(),
        });
        let report = run(&[], unlicensed);
        assert!(matches!(
            report.metrics.baseline,
            BaselineComparison::Unavailable { .. }
        ));

        let mut request = request();
        request.instrument = Some("TEST".into());
        request.baseline = Some(MarketBaseline {
            instrument: "TEST".into(),
            as_of: "2024-01-01".into(),
            licensed: true,
            median_close: 100.0,
            source: "fixture".into(),
        });
        let rows = vec![
            row(100.0, 102.0, 99.0, 101.0),
            row(101.0, 103.0, 100.0, 102.0),
            row(102.0, 104.0, 101.0, 103.0),
        ];
        let report = run(&rows, request);
        assert!(matches!(
            report.metrics.baseline,
            BaselineComparison::Available { .. }
        ));
    }

    #[test]
    fn invalid_thresholds_are_rejected() {
        let mut request = request();
        request.thresholds.price_jump_ratio = 1.0;
        assert!(request.validate().is_err());
    }
}
