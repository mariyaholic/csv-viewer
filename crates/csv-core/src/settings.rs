// SPDX-License-Identifier: Apache-2.0

//! Per-user settings. Local only: no telemetry, no network, no registry
//! writes on non-Windows platforms.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dialect::{Dialect, DialectChoice};
use crate::encoding::{ChoiceSource, EncodingKind};
use crate::market::MarketRequest;
use crate::model::{EncodingChoice, HeaderChoice, ScanRequest};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// `None` means auto-detect.
    pub delimiter: Option<char>,
    pub quote: Option<char>,
    /// Label accepted by [`EncodingKind::parse`], e.g. `utf-8`.
    pub encoding: Option<String>,
    /// `None` means auto-detect; otherwise an explicit header choice.
    pub header: Option<HeaderChoice>,
    pub preview_rows: usize,
    pub max_field_chars: usize,
    /// Optional, user-mapped market profile. Disabled by default.
    pub market: Option<MarketRequest>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            delimiter: None,
            quote: None,
            encoding: None,
            header: None,
            preview_rows: 200,
            max_field_chars: 4096,
            market: None,
        }
    }
}

impl Settings {
    pub fn load(path: Option<&Path>) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let path = match path.map(Path::to_path_buf).or_else(default_path) {
            Some(path) => path,
            None => {
                warnings.push("no per-user settings directory could be determined".to_string());
                return (Self::default(), warnings);
            }
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(settings) => {
                    if let Err(message) = settings.validate() {
                        warnings.push(format!("settings were ignored: {message}"));
                        return (Self::default(), warnings);
                    }
                    warnings.push(format!("loaded settings from {}", path.display()));
                    (settings, warnings)
                }
                Err(error) => {
                    warnings.push(format!(
                        "settings at {} could not be parsed ({error}); defaults used",
                        path.display()
                    ));
                    (Self::default(), warnings)
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (Self::default(), warnings)
            }
            Err(error) => {
                warnings.push(format!(
                    "settings at {} could not be read ({error}); defaults used",
                    path.display()
                ));
                (Self::default(), warnings)
            }
        }
    }

    /// Preview paths must not silently certify data using fallback settings.
    /// Only a genuinely missing file selects defaults; every other problem
    /// is surfaced to the caller before parsing the source.
    pub fn load_checked(path: Option<&Path>) -> Result<Self, String> {
        use std::io::Read;
        let path = path
            .map(Path::to_path_buf)
            .or_else(default_path)
            .ok_or_else(|| "no per-user settings directory could be determined".to_string())?;
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => {
                return Err(format!(
                    "settings at {} could not be read: {error}",
                    path.display()
                ))
            }
        };
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!("settings at {} could not be read: {error}", path.display())
            })?;
        if bytes.len() > 1024 * 1024 {
            return Err("settings exceed 1 MiB".into());
        }
        let settings: Self = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "settings at {} could not be parsed: {error}",
                path.display()
            )
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let parent = path
            .parent()
            .ok_or_else(|| "settings path has no parent directory".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialise settings: {error}"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("could not create a temporary settings file: {error}"))?;
        use std::io::Write;
        temporary
            .write_all(text.as_bytes())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| format!("could not write settings: {error}"))?;
        temporary
            .persist(path)
            .map_err(|error| format!("could not replace settings: {}", error.error))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.quote.is_some() && self.delimiter.is_none() {
            return Err("a quote override requires an explicit delimiter".into());
        }
        if self.preview_rows == 0 || self.preview_rows > 10_000 {
            return Err("preview_rows must be between 1 and 10000".to_string());
        }
        if self.max_field_chars < 16 || self.max_field_chars > 1_000_000 {
            return Err("max_field_chars must be between 16 and 1000000".to_string());
        }
        if let Some(encoding) = &self.encoding {
            if EncodingKind::parse(encoding).is_none() {
                return Err(format!("unsupported encoding label {encoding:?}"));
            }
        }
        if let Some(delimiter) = self.delimiter {
            let dialect = Dialect {
                delimiter,
                quote: self.quote.unwrap_or('"'),
                ..Dialect::default()
            };
            dialect.validate()?;
        }
        if let Some(market) = &self.market {
            market.validate()?;
        }
        Ok(())
    }

    pub fn dialect_choice(&self) -> DialectChoice {
        match self.delimiter {
            None => DialectChoice::Auto,
            Some(delimiter) => DialectChoice::Explicit(Dialect {
                delimiter,
                quote: self.quote.unwrap_or('"'),
                ..Dialect::default()
            }),
        }
    }

    pub fn encoding_choice(&self) -> EncodingChoice {
        match &self.encoding {
            None => EncodingChoice::Auto,
            Some(label) => match EncodingKind::parse(label) {
                Some(kind) => EncodingChoice::Explicit(kind),
                None => EncodingChoice::Auto,
            },
        }
    }

    pub fn header_choice(&self) -> HeaderChoice {
        self.header.unwrap_or(HeaderChoice::Auto)
    }

    pub fn apply_to(&self, request: &mut ScanRequest) {
        request.dialect = self.dialect_choice();
        request.encoding = self.encoding_choice();
        request.header = self.header_choice();
        request.preview_rows = self.preview_rows;
        request.limits.max_field_chars = self.max_field_chars;
        request.market = self.market.clone();
    }

    /// Describe where a choice came from for UI reporting.
    pub fn encoding_source(&self) -> ChoiceSource {
        if self.encoding.is_some() {
            ChoiceSource::Override
        } else {
            ChoiceSource::Auto
        }
    }
}

/// The per-user settings path. `CSV_VIEWER_SETTINGS` is an explicit test and
/// portability override; it is never read from the network.
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CSV_VIEWER_SETTINGS") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    #[cfg(windows)]
    {
        let base = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(base).join("csv-viewer").join("settings.json"))
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
            if !base.is_empty() {
                return Some(PathBuf::from(base).join("csv-viewer").join("settings.json"));
            }
        }
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("csv-viewer")
                .join("settings.json"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_load_rejects_misspelled_keys_and_ignored_quote_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for bytes in [
            br#"{"delimter":";"}"#.as_slice(),
            br#"{"encodng":"utf-8"}"#.as_slice(),
            br#"{"quote":"'"}"#.as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(Settings::load_checked(Some(&path)).is_err());
            let (_, warnings) = Settings::load(Some(&path));
            assert!(!warnings.is_empty(), "legacy load must surface fallback");
        }
        std::fs::write(&path, br#"{"delimiter":";","quote":"'"}"#).unwrap();
        assert!(Settings::load_checked(Some(&path)).is_ok());
    }

    #[test]
    fn checked_load_rejects_errors_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(
            Settings::load_checked(Some(&path)).unwrap(),
            Settings::default()
        );
        for bytes in [
            b"broken".as_slice(),
            br#"{"encoding":"not-an-encoding"}"#.as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(Settings::load_checked(Some(&path)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        assert!(Settings::load_checked(Some(dir.path())).is_err());
    }

    #[test]
    fn default_settings_are_valid() {
        Settings::default().validate().unwrap();
    }

    #[test]
    fn roundtrip_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let settings = Settings {
            delimiter: Some(';'),
            encoding: Some("windows-1252".to_string()),
            header: Some(HeaderChoice::Absent),
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        let (loaded, warnings) = Settings::load(Some(&path));
        assert_eq!(loaded, settings);
        assert!(warnings.iter().any(|warning| warning.starts_with("loaded")));
    }

    #[test]
    fn corrupt_settings_fall_back_to_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        let (loaded, warnings) = Settings::load(Some(&path));
        assert_eq!(loaded, Settings::default());
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("could not be parsed")));
    }

    #[test]
    fn missing_settings_are_silent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("does-not-exist.json");
        let (loaded, warnings) = Settings::load(Some(&path));
        assert_eq!(loaded, Settings::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn choices_are_applied() {
        let settings = Settings {
            delimiter: Some('\t'),
            encoding: Some("utf-16le".to_string()),
            header: Some(HeaderChoice::Present),
            ..Settings::default()
        };
        let mut request = ScanRequest::default();
        settings.apply_to(&mut request);
        assert_eq!(request.preview_rows, 200);
        assert!(matches!(request.dialect, DialectChoice::Explicit(_)));
        assert!(matches!(
            request.encoding,
            crate::model::EncodingChoice::Explicit(_)
        ));
        assert_eq!(request.header, HeaderChoice::Present);
    }

    #[test]
    fn invalid_values_are_rejected() {
        let mut settings = Settings {
            preview_rows: 0,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
        settings.preview_rows = 200;
        settings.encoding = Some("klingon".to_string());
        assert!(settings.validate().is_err());
    }
}
