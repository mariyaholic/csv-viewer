// SPDX-License-Identifier: Apache-2.0

//! Plain-text rendering shared by the preview handler and the non-TTY viewer.
//!
//! Rendering is deliberately text-only: no formulas are evaluated, no links
//! are opened, and terminal control characters are escaped so a hostile file
//! cannot drive the terminal.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::ScanReport;
use crate::warnings::Warning;

/// Replace terminal-hostile control and bidi characters, and collapse line
/// breaks/tabs into visible escapes. Source text itself is never modified; this
/// is display only.
pub fn sanitize_display(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            ch if ch.is_control() => result.push('�'),
            '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => {
                result.push('�')
            }
            ch => result.push(ch),
        }
    }
    result
}

/// Truncate to a display width, appending `…` when something was removed.
pub fn truncate_display(value: &str, width: usize) -> (String, bool) {
    let sanitized = sanitize_display(value);
    if width == 0 {
        return (String::new(), !sanitized.is_empty());
    }
    if sanitized.width() <= width {
        return (sanitized, false);
    }
    let mut result = String::new();
    let mut used = 0usize;
    for ch in sanitized.chars() {
        let char_width = ch.width().unwrap_or(0);
        if used + char_width + 1 > width {
            break;
        }
        result.push(ch);
        used += char_width;
    }
    result.push('…');
    (result, true)
}

pub fn pad_display(value: &str, width: usize) -> String {
    let (value, _) = truncate_display(value, width);
    let used = value.width();
    let mut result = value;
    if used < width {
        result.push_str(&" ".repeat(width - used));
    }
    result
}

/// Render a bounded preview as a plain-text page.
pub fn render_text(report: &ScanReport, width: usize, max_rows: usize) -> String {
    let width = width.max(40);
    let mut lines = Vec::new();
    lines.push(truncate_display("CSV viewer and preview — Made by Maria", width).0);
    lines.push(String::new());

    let bom = if report.encoding.bom {
        ", BOM removed"
    } else {
        ""
    };
    lines.push(format!(
        "Encoding: {}{}   Delimiter: {:?}   Header: {}",
        report.encoding.label,
        bom,
        report.dialect.delimiter,
        if report.header.present { "yes" } else { "no" }
    ));
    lines.push(format!("Coverage: {}", report.coverage_summary()));
    if report.complete {
        if let Some(total) = report.total_records {
            lines.push(format!("Total records: {total}"));
        }
    } else {
        lines.push(
            "Incomplete scan: no exact total is shown; only the scanned records below.".to_string(),
        );
    }
    lines.push(String::new());

    if report.columns.is_empty() {
        lines.push("(no records)".to_string());
    } else {
        let widths = column_widths(report, width, max_rows);
        let mut header_cells = Vec::new();
        for (index, column) in report.columns.iter().enumerate() {
            if let Some(column_width) = widths.get(index) {
                header_cells.push(pad_display(&column.name, *column_width));
            }
        }
        if header_cells.len() < report.columns.len() {
            header_cells.push("…".to_string());
        }
        lines.push(format!("{:>5}  {}", "#", header_cells.join("  ")));
        let rule = 7 + header_cells.join("  ").width();
        lines.push("-".repeat(rule.min(width)));
        for row in report.preview_rows.iter().take(max_rows) {
            let mut cells = Vec::new();
            for (index, column_width) in widths.iter().enumerate() {
                let value = row.cells.get(index).map(String::as_str).unwrap_or("");
                cells.push(pad_display(value, *column_width));
            }
            if widths.len() < report.columns.len() {
                cells.push("…".to_string());
            }
            lines.push(format!("{:>5}  {}", row.record, cells.join("  ")));
        }
        if report.preview_rows.len() > max_rows {
            lines.push(format!(
                "… {} more preview rows not shown",
                report.preview_rows.len() - max_rows
            ));
        }
        lines.push(String::new());
    }

    if !report.date_ranges.is_empty() {
        for range in &report.date_ranges {
            match range.status {
                crate::model::DateRangeStatus::Computed => {
                    lines.push(format!(
                        "Date range: {} ({}) = {} .. {}; {} parsed, {} invalid, {} missing",
                        sanitize_display(range.column_name.as_deref().unwrap_or("column ?")),
                        range.format.description(),
                        range.min.as_deref().unwrap_or("?"),
                        range.max.as_deref().unwrap_or("?"),
                        range.parsed,
                        range.invalid,
                        range.missing
                    ));
                }
                crate::model::DateRangeStatus::Unavailable => {
                    lines.push(format!(
                        "Date range: {} — unavailable: {}",
                        sanitize_display(range.column_name.as_deref().unwrap_or("column ?")),
                        range.reason.as_deref().unwrap_or("not available")
                    ));
                }
            }
        }
        lines.push(String::new());
    }

    if !report.warnings.is_empty() {
        lines.push(format!("Review notes ({}):", report.warnings.len()));
        for warning in report.warnings.iter().take(16) {
            lines.push(format_warning(warning, width));
        }
        if report.warnings.len() > 16 {
            lines.push(format!(
                "… {} more review notes",
                report.warnings.len() - 16
            ));
        }
        lines.push(String::new());
    }

    if let Some(market) = &report.market {
        lines.push("Market profile (optional heuristics):".to_string());
        if !market.available {
            lines.push(format!(
                "  unavailable: {}",
                market
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("unknown reason")
            ));
        } else {
            lines.push(format!(
                "  {} valid rows, {} rows missing fields{}",
                market.sample_rows,
                market.missing_rows,
                if market.coverage_incomplete {
                    " (partial scan)"
                } else {
                    ""
                }
            ));
            for finding in market.findings.iter().take(8) {
                lines.push(format!(
                    "  - {:?} ({} of {}; threshold {:.6})",
                    finding.code, finding.count, finding.sample_size, finding.threshold
                ));
            }
        }
        lines.push(format!("  {}", market.disclaimer));
        lines.push(String::new());
    }

    lines.push("Read-only. Formulas are never executed and links are never opened.".to_string());
    lines.push("Made by Maria · free and open source (Apache-2.0)".to_string());
    lines.join("\n")
}

fn column_widths(report: &ScanReport, width: usize, max_rows: usize) -> Vec<usize> {
    let number_width = 7;
    let available = width.saturating_sub(number_width);
    let mut widths = Vec::new();
    for (index, column) in report.columns.iter().enumerate() {
        let mut needed = column.name.width().max(4);
        for row in report.preview_rows.iter().take(max_rows) {
            if let Some(cell) = row.cells.get(index) {
                needed = needed.max(cell.width().min(30));
            }
        }
        let needed = needed.clamp(4, 30);
        let spacing = if widths.is_empty() { 0 } else { 2 };
        let used: usize = widths.iter().sum::<usize>() + spacing * widths.len();
        if used + needed > available {
            break;
        }
        widths.push(needed);
    }
    if widths.is_empty() {
        widths.push(available.saturating_sub(2).max(4));
    }
    widths
}

fn format_warning(warning: &Warning, width: usize) -> String {
    let column = warning
        .column_name
        .as_deref()
        .map(|name| format!(" [{name}]"))
        .unwrap_or_default();
    let sample = warning
        .samples
        .first()
        .map(|sample| format!(" e.g. {:?}", sample.value))
        .unwrap_or_default();
    let marker = match warning.severity {
        crate::warnings::Severity::Info => "i",
        crate::warnings::Severity::Warning => "!",
    };
    let text = format!(
        "{marker} {:?}{column} ×{}: {}{sample}",
        warning.code, warning.occurrences, warning.message
    );
    truncate_display(&text, width).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::scan::scan;
    use std::io;

    fn report() -> ScanReport {
        let mut request = ScanRequest::full();
        request.header = HeaderChoice::Present;
        scan(
            io::Cursor::new(b"name,value\n007,=SUM(A1)\n".to_vec()),
            InputInfo::known(27),
            &request,
        )
        .unwrap()
    }

    #[test]
    fn rule_matches_header_width() {
        for width in [40, 80, 160] {
            let page = render_text(&report(), width, 10);
            let lines: Vec<_> = page.lines().collect();
            let rule = lines
                .iter()
                .position(|line| line.starts_with("---"))
                .unwrap();
            assert_eq!(lines[rule].width(), lines[rule - 1].width().min(width));
        }
    }

    #[test]
    fn sanitizes_terminal_controls() {
        let rendered = sanitize_display("a\u{1b}[31mred\nnext\tcell");
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains("\\n"));
        assert!(rendered.contains("\\t"));
    }

    #[test]
    fn sanitizes_date_range_column_names() {
        let mut request = ScanRequest::full();
        request.header = HeaderChoice::Present;
        request.date_ranges = vec![DateRangeRequest {
            column: 0,
            format: crate::types::DateFormat::Ymd,
        }];
        let report = scan(
            io::Cursor::new(b"\x1b[31mred,value\n2024-01-02,3\n".to_vec()),
            InputInfo::known(30),
            &request,
        )
        .unwrap();
        let text = render_text(&report, 80, 10);
        assert!(
            text.contains("Date range:"),
            "date-range lines must render: {text}"
        );
        assert!(
            !text.contains('\u{1b}'),
            "rendered page must not carry a raw ESC byte: {text:?}"
        );
    }

    #[test]
    fn truncates_to_display_width() {
        assert_eq!(truncate_display("abcdef", 3).0, "ab…");
        assert_eq!(truncate_display("abc", 3).0, "abc");
        assert_eq!(pad_display("ab", 4), "ab  ");
    }

    #[test]
    fn rendering_marks_partial_scans_without_a_total() {
        let mut request = ScanRequest::preview();
        request.header = HeaderChoice::Present;
        let report = scan(
            io::Cursor::new(b"name,value\n1,2\n3,4\n".to_vec()),
            InputInfo::preview(Some(20)),
            &request,
        )
        .unwrap();
        let text = render_text(&report, 80, 10);
        assert!(text.contains("Incomplete scan"));
        assert!(!text.contains("Total records:"));
        assert!(text.contains("Made by Maria"));
        assert!(text.contains("read-only") || text.contains("Read-only"));
    }

    #[test]
    fn rendering_shows_warning_columns() {
        let text = render_text(&report(), 100, 10);
        assert!(text.contains("LeadingZeros") || text.contains("leading zeros"));
        assert!(text.contains("FormulaLikeText") || text.contains("formula"));
    }
}
