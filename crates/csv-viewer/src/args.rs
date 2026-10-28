// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use csvview_core::{DateFormat, EncodingKind, HeaderChoice};

#[derive(Debug, Clone)]
pub struct Options {
    pub path: PathBuf,
    pub render: bool,
    pub full: bool,
    pub rows: usize,
    pub delimiter: Option<char>,
    pub quote: Option<char>,
    pub encoding: Option<EncodingKind>,
    pub header: Option<HeaderChoice>,
    pub settings: Option<PathBuf>,
    /// Zero-based column index after parsing a 1-based command-line value.
    pub date_column: Option<usize>,
    pub date_format: Option<DateFormat>,
}

impl Options {
    pub fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self {
            path: PathBuf::new(),
            render: false,
            full: false,
            rows: 200,
            delimiter: None,
            quote: None,
            encoding: None,
            header: None,
            settings: None,
            date_column: None,
            date_format: None,
        };
        let mut path = None;
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--render" => options.render = true,
                "--full" => options.full = true,
                "--rows" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--rows needs a value".to_string())?;
                    options.rows = value
                        .parse::<usize>()
                        .map_err(|_| "--rows must be a positive integer".to_string())?;
                    if options.rows == 0 {
                        return Err("--rows must be greater than zero".to_string());
                    }
                }
                "--delimiter" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--delimiter needs a value".to_string())?;
                    options.delimiter = Some(single_ascii(&value, "--delimiter")?);
                }
                "--quote" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--quote needs a value".to_string())?;
                    options.quote = Some(single_ascii(&value, "--quote")?);
                }
                "--encoding" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--encoding needs a value".to_string())?;
                    options.encoding = Some(
                        EncodingKind::parse(&value)
                            .ok_or_else(|| format!("unsupported encoding label {value:?}"))?,
                    );
                }
                "--header" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--header needs a value".to_string())?;
                    options.header = Some(match value.to_ascii_lowercase().as_str() {
                        "auto" => HeaderChoice::Auto,
                        "present" | "yes" => HeaderChoice::Present,
                        "absent" | "no" => HeaderChoice::Absent,
                        _ => return Err("--header must be auto, present or absent".to_string()),
                    });
                }
                "--no-header" => options.header = Some(HeaderChoice::Absent),
                "--settings" => {
                    options.settings = Some(PathBuf::from(
                        arguments
                            .next()
                            .ok_or_else(|| "--settings needs a path".to_string())?,
                    ));
                }
                "--date-column" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--date-column needs a value".to_string())?;
                    let column = value
                        .parse::<usize>()
                        .map_err(|_| "--date-column must be a 1-based integer".to_string())?;
                    if column == 0 {
                        return Err("--date-column is 1-based and must be at least 1".to_string());
                    }
                    options.date_column = Some(column - 1);
                }
                "--date-format" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--date-format needs a value".to_string())?;
                    options.date_format = Some(match value.to_ascii_lowercase().as_str() {
                        "ymd" => DateFormat::Ymd,
                        "dmy" => DateFormat::Dmy,
                        "mdy" => DateFormat::Mdy,
                        _ => return Err("--date-format must be ymd, dmy or mdy".to_string()),
                    });
                }
                "--help" | "-h" => {
                    println!(
                        "csv-viewer {}\n\nUsage: csv-viewer [options] <file.csv>\n\n\
                         Options:\n  --render           print a plain-text preview and exit\n  \
                         --full             run one complete scan before showing the view\n  \
                         --rows N           preview rows (default 200)\n  \
                         --delimiter C      override one-character delimiter\n  \
                         --quote C          override one-character quote\n  \
                         --encoding LABEL   utf-8, utf-16le, utf-16be or windows-1252\n  \
                         --header MODE      auto, present or absent\n  \
                         --no-header        shorthand for --header absent\n  \
                         --settings PATH    per-user settings JSON\n  \
                         --date-column N    selected 1-based column for a date range\n  \
                         --date-format F    ymd, dmy or mdy (required with --date-column)\n\n\
                         Read-only. No network, telemetry, editing or SQL.\n\
                         Made by Maria. free and open source (Apache-2.0).",
                        env!("CARGO_PKG_VERSION")
                    );
                    std::process::exit(0);
                }
                "--version" => {
                    println!("csv-viewer {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown argument {value:?}"));
                }
                value => {
                    if path.is_some() {
                        return Err("only one input path is supported".to_string());
                    }
                    path = Some(PathBuf::from(value));
                }
            }
        }
        options.path = path.ok_or_else(|| "an input path is required".to_string())?;
        if options.date_column.is_some() != options.date_format.is_some() {
            return Err("--date-column and --date-format must be used together".to_string());
        }
        Ok(options)
    }
}

fn single_ascii(value: &str, name: &str) -> Result<char, String> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(format!("{name} needs one ASCII character"));
    };
    if chars.next().is_some() || !ch.is_ascii() || ch.is_ascii_control() {
        return Err(format!("{name} needs one printable ASCII character"));
    }
    Ok(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<Options, String> {
        Options::parse(values.iter().map(|value| value.to_string()))
    }

    #[test]
    fn parses_a_path_and_flags() {
        let options = parse(&["--render", "--full", "--rows", "10", "data.csv"]).unwrap();
        assert_eq!(options.path, PathBuf::from("data.csv"));
        assert!(options.render);
        assert!(options.full);
        assert_eq!(options.rows, 10);
    }

    #[test]
    fn parses_overrides() {
        let options = parse(&[
            "--delimiter",
            ";",
            "--quote",
            "'",
            "--encoding",
            "cp1252",
            "--header",
            "absent",
            "x.csv",
        ])
        .unwrap();
        assert_eq!(options.delimiter, Some(';'));
        assert_eq!(options.quote, Some('\''));
        assert_eq!(options.encoding, Some(EncodingKind::Windows1252));
        assert_eq!(options.header, Some(HeaderChoice::Absent));
    }

    #[test]
    fn parses_a_date_range_request() {
        let options = parse(&["--date-column", "3", "--date-format", "dmy", "x.csv"]).unwrap();
        assert_eq!(options.date_column, Some(2));
        assert_eq!(options.date_format, Some(DateFormat::Dmy));
        assert!(parse(&["--date-column", "0", "--date-format", "dmy", "x.csv"]).is_err());
        assert!(parse(&["--date-column", "2", "x.csv"]).is_err());
    }

    #[test]
    fn rejects_bad_values() {
        assert!(parse(&["--rows", "0", "x.csv"]).is_err());
        assert!(parse(&["--delimiter", "ab", "x.csv"]).is_err());
        assert!(parse(&["--encoding", "klingon", "x.csv"]).is_err());
        assert!(parse(&["--header", "maybe", "x.csv"]).is_err());
        assert!(parse(&["a.csv", "b.csv"]).is_err());
        assert!(parse(&[]).is_err());
    }
}
