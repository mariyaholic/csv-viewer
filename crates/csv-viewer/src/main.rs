// SPDX-License-Identifier: Apache-2.0

//! Small companion viewer. Read-only: it never edits the source, executes
//! formulas, opens links, or uses the network.

mod app;
mod args;
mod ui;

use csvview_core::{
    scan_file, DateRangeRequest, Dialect, DialectChoice, EncodingChoice, ScanLimits, ScanRequest,
    Settings,
};
use std::io::IsTerminal;

use crate::app::App;
use crate::args::Options;

fn main() {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("csv-viewer: {message}");
            std::process::exit(2);
        }
    };
    let (settings, warnings) = Settings::load(options.settings.as_deref());
    for warning in &warnings {
        eprintln!("csv-viewer: {warning}");
    }
    let request = build_request(&options, &settings);
    let scan_request = if options.full {
        full_request(&request)
    } else {
        request.clone()
    };
    let report = match scan_file(&options.path, &scan_request) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("csv-viewer: {error}");
            std::process::exit(1);
        }
    };

    let text_mode = options.render || !std::io::stdout().is_terminal();
    if text_mode {
        let width = terminal_width().unwrap_or(100);
        println!("{}", csvview_core::text::render_text(&report, width, 80));
        return;
    }

    let app = App::new(report, options.path.clone(), request);
    if let Err(error) = ui::run(app) {
        eprintln!("csv-viewer: terminal error: {error}");
        // A terminal failure should still leave the user with a readable view.
        if let Ok(report) = scan_file(&options.path, &scan_request) {
            println!("{}", csvview_core::text::render_text(&report, 100, 80));
        }
        std::process::exit(1);
    }
}

fn build_request(options: &Options, settings: &Settings) -> ScanRequest {
    let mut request = ScanRequest::preview();
    settings.apply_to(&mut request);

    let delimiter = options.delimiter.or(settings.delimiter);
    let quote = options.quote.or(settings.quote).unwrap_or('"');
    if options.delimiter.is_some() || options.quote.is_some() || delimiter.is_some() {
        request.dialect = DialectChoice::Explicit(Dialect {
            delimiter: delimiter.unwrap_or(','),
            quote,
            ..Dialect::default()
        });
    }
    if let Some(encoding) = options.encoding {
        request.encoding = EncodingChoice::Explicit(encoding);
    }
    if let Some(header) = options.header {
        request.header = header;
    }
    if let (Some(column), Some(format)) = (options.date_column, options.date_format) {
        request
            .date_ranges
            .push(DateRangeRequest { column, format });
    }
    request.preview_rows = options.rows;
    request.limits.max_records = Some(options.rows as u64);
    request.limits.max_bytes = None;
    request.limits.deadline_ms = Some(5_000);
    request
}

fn full_request(request: &ScanRequest) -> ScanRequest {
    let mut request = request.clone();
    request.limits = ScanLimits::full();
    request
}

fn terminal_width() -> Option<usize> {
    crossterm::terminal::size()
        .ok()
        .map(|(width, _)| width as usize)
        .filter(|width| *width >= 40)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_overrides_settings() {
        let mut settings = Settings::default();
        settings.delimiter = Some(';');
        let options = Options::parse(
            ["--delimiter", "|", "--encoding", "utf-16le", "x.csv"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        let request = build_request(&options, &settings);
        match request.dialect {
            DialectChoice::Explicit(dialect) => assert_eq!(dialect.delimiter, '|'),
            other => panic!("expected explicit dialect, got {other:?}"),
        }
        assert!(matches!(request.encoding, EncodingChoice::Explicit(_)));
        assert_eq!(request.limits.max_records, Some(200));
    }

    #[test]
    fn date_range_request_is_forwarded() {
        let options = Options::parse(
            ["--date-column", "2", "--date-format", "ymd", "x.csv"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        let request = build_request(&options, &Settings::default());
        assert_eq!(request.date_ranges.len(), 1);
        assert_eq!(request.date_ranges[0].column, 1);
    }

    #[test]
    fn full_request_is_bounded_and_explicit() {
        let options = Options::parse(["x.csv".to_string()].into_iter()).unwrap();
        let request = full_request(&build_request(&options, &Settings::default()));
        assert_eq!(request.limits.max_records, ScanLimits::full().max_records);
        assert_eq!(request.limits.deadline_ms, ScanLimits::full().deadline_ms);
    }
}
