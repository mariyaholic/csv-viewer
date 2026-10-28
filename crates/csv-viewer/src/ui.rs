// SPDX-License-Identifier: Apache-2.0

use std::io::{self, Write};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use csvview_core::text::{pad_display, sanitize_display, truncate_display};
use csvview_core::DateRangeStatus;
use unicode_width::UnicodeWidthStr;

use crate::app::{Action, App};

pub fn run(mut app: App) -> io::Result<()> {
    let _guard = TerminalGuard::enter()?;
    loop {
        let (width, height) = terminal::size()?;
        draw(&mut app, width as usize, height as usize)?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.handle_key(key) {
                        Action::Quit => break,
                        Action::FullScan => app.start_full_scan(),
                        Action::None => {}
                    }
                }
            }
        }
        app.poll_full_scan();
    }
    Ok(())
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn draw(app: &mut App, width: usize, height: usize) -> io::Result<()> {
    let width = width.max(40);
    let height = height.max(10);
    let body_rows = height.saturating_sub(8).max(1);
    let column_guess = ((width.saturating_sub(8)) / 14).max(1);
    app.set_viewport(body_rows, column_guess);

    let mut lines = Vec::new();
    lines.push(truncate_display("CSV viewer and preview — Made by Maria", width).0);
    let path = truncate_display(&app.path.display().to_string(), width).0;
    lines.push(path);
    let mut status = format!(
        "{} · {} · header {} · {}",
        app.report.encoding.label,
        format!("delimiter {:?}", app.report.dialect.delimiter),
        if app.report.header.present { "yes" } else { "no" },
        app.report.coverage_summary()
    );
    if app.scanning {
        status.push_str(" · full scan running…");
    }
    if let Some(message) = &app.message {
        if !app.scanning {
            status.push_str(" · ");
            status.push_str(message);
        }
    }
    lines.push(truncate_display(&status, width).0);

    if app.show_about {
        lines.push(String::new());
        for line in about_lines() {
            lines.push(truncate_display(line, width).0);
        }
    } else if app.show_details {
        lines.push(String::new());
        lines.push(truncate_display("Scan details", width).0);
        for line in detail_lines(app) {
            lines.push(truncate_display(&line, width).0);
        }
    } else if app.show_warnings {
        lines.push(String::new());
        lines.push(truncate_display("Review notes (facts about the source; no conversion is made)", width).0);
        if app.report.warnings.is_empty() {
            lines.push("No review notes for the scanned records.".to_string());
        } else {
            let available = height.saturating_sub(lines.len() + 2);
            for warning in app
                .report
                .warnings
                .iter()
                .skip(app.warning_scroll)
                .take(available)
            {
                let column = warning
                    .column_name
                    .as_deref()
                    .map(|name| format!(" [{name}]"))
                    .unwrap_or_default();
                lines.push(truncate_display(
                    &format!(
                        "! {:?}{column} ×{}: {}",
                        warning.code, warning.occurrences, warning.message
                    ),
                    width,
                ).0);
                if let Some(sample) = warning.samples.first() {
                    lines.push(truncate_display(
                        &format!("    e.g. {:?}", sample.value),
                        width,
                    ).0);
                }
            }
        }
    } else {
        lines.push(String::new());
        render_table(app, width, body_rows, &mut lines);
    }

    while lines.len() < height.saturating_sub(2) {
        lines.push(String::new());
    }
    lines.push(truncate_display(
        "q quit · ↑↓ scroll · ←→ columns · PgUp/PgDn · w warnings · d details · a about · f full scan",
        width,
    ).0);
    lines.push(truncate_display(
        "Read-only · formulas never executed · links never opened · Made by Maria · Apache-2.0",
        width,
    ).0);

    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    for line in lines.iter().take(height) {
        let line = truncate_display(line, width).0;
        let padding = width.saturating_sub(line.width());
        writeln!(stdout, "{line}{}", " ".repeat(padding))?;
    }
    stdout.flush()
}

fn render_table(app: &App, width: usize, body_rows: usize, lines: &mut Vec<String>) {
    let visible_columns = app.table.visible_columns();
    let mut widths: Vec<usize> = Vec::new();
    let available = width.saturating_sub(7);
    for index in visible_columns.clone() {
        let header = app
            .table
            .columns
            .get(index)
            .map(String::as_str)
            .unwrap_or("");
        let mut needed = header.width().max(5);
        for row in app.table.visible_rows() {
            if let Some(cell) = app.table.cell(row, index) {
                needed = needed.max(sanitize_display(cell).width().min(30));
            }
        }
        let needed = needed.min(30).max(5);
        let used: usize = widths.iter().sum::<usize>() + 2 * widths.len();
        if used + needed > available {
            break;
        }
        widths.push(needed);
    }
    if widths.is_empty() {
        widths.push(available.saturating_sub(2).max(5));
    }
    let mut header = format!("{:>5}  ", "#");
    for (offset, width) in widths.iter().enumerate() {
        let index = visible_columns.start + offset;
        let name = app
            .table
            .columns
            .get(index)
            .map(String::as_str)
            .unwrap_or("");
        header.push_str(&pad_display(name, *width));
        header.push_str("  ");
    }
    lines.push(truncate_display(header.trim_end(), width).0);
    lines.push("-".repeat(width.min(120)));

    let visible_rows = app.table.visible_rows();
    if visible_rows.is_empty() {
        lines.push("(no preview rows)".to_string());
        return;
    }
    for row in visible_rows {
        let mut line = format!("{:>5}  ", row + 1);
        for (offset, width) in widths.iter().enumerate() {
            let column = visible_columns.start + offset;
            let cell = app.table.cell(row, column).unwrap_or("");
            line.push_str(&pad_display(cell, *width));
            line.push_str("  ");
        }
        lines.push(truncate_display(line.trim_end(), width).0);
        if lines.len() >= body_rows + 3 {
            break;
        }
    }
}

fn detail_lines(app: &App) -> Vec<String> {
    let report = &app.report;
    let mut lines = vec![
        format!("Coverage: {}", report.coverage_summary()),
        format!(
            "Encoding: {} ({:?}, BOM {})",
            report.encoding.label, report.encoding.source, report.encoding.bom
        ),
        format!(
            "Dialect: delimiter {:?}, quote {:?}, confidence {}",
            report.dialect.delimiter, report.dialect.quote, report.dialect.confidence
        ),
        format!(
            "Header: {} ({} columns)",
            if report.header.present { "yes" } else { "no" },
            report.columns.len()
        ),
        format!("Review notes: {}", report.warnings.len()),
    ];
    if report.date_ranges.is_empty() {
        lines.push("Date range: none requested (--date-column/--date-format)".to_string());
    }
    for range in &report.date_ranges {
        lines.push(match range.status {
            DateRangeStatus::Computed => format!(
                "Date range: {} ({}) {} .. {}; {} parsed, {} invalid, {} missing",
                range.column_name.as_deref().unwrap_or("?"),
                range.format.description(),
                range.min.as_deref().unwrap_or("?"),
                range.max.as_deref().unwrap_or("?"),
                range.parsed,
                range.invalid,
                range.missing
            ),
            DateRangeStatus::Unavailable => format!(
                "Date range: {} unavailable — {}",
                range.column_name.as_deref().unwrap_or("?"),
                range.reason.as_deref().unwrap_or("not available")
            ),
        });
    }
    if let Some(market) = &report.market {
        if market.available {
            lines.push(format!(
                "Market profile: {} valid rows, {} missing; complete={}",
                market.sample_rows,
                market.missing_rows,
                !market.coverage_incomplete
            ));
            for finding in &market.findings {
                lines.push(format!(
                    "  {:?}: {} of {} (threshold {:.6})",
                    finding.code, finding.count, finding.sample_size, finding.threshold
                ));
            }
        } else {
            lines.push(format!(
                "Market profile unavailable: {}",
                market.unavailable_reason.as_deref().unwrap_or("unknown")
            ));
        }
        lines.push(format!("  {}", market.disclaimer));
    }
    lines.push("Full scan is only run when requested (f or --full).".to_string());
    lines
}

pub fn about_lines() -> Vec<&'static str> {
    vec![
        "CSV viewer and preview",
        "Made by Maria",
        "free and open source (Apache-2.0)",
        "",
        concat!("Version ", env!("CARGO_PKG_VERSION")),
        "",
        "Inspect CSV text without converting it.",
        "The first screen is a bounded preview; a complete scan is explicit.",
        "Partial scans show scanned records and byte coverage, never an exact total.",
        "",
        "Read-only. No editing, SQL, telemetry or network access.",
        "Formulas are rendered as text and never executed; links are never opened.",
        "Optional market heuristics are not proof of fraud and are not financial advice.",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_names_the_maker_and_licence() {
        let text = about_lines().join("\n");
        assert!(text.contains("Made by Maria"));
        assert!(text.contains("Apache-2.0"));
        assert!(text.contains("never executed"));
    }
}
