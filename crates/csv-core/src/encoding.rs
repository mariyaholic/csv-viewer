// SPDX-License-Identifier: Apache-2.0

//! Encoding detection and streaming decoding.
//!
//! v1 deliberately supports a small, explicit set: UTF-8, UTF-16LE, UTF-16BE
//! and Windows-1252. Any other encoding can be supplied as an override only
//! after it is added here. Detection never rewrites the source; decoding is
//! streamed and replacement errors are recorded rather than hidden.

use std::io::{self, BufReader, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use encoding_rs::{CoderResult, Decoder, Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};
use serde::{Deserialize, Serialize};

/// Bytes inspected for detection and dialect/header inference.
pub const SAMPLE_BYTES: usize = 64 * 1024;

/// How an encoding/dialect was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChoiceSource {
    Auto,
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EncodingKind {
    Utf8,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl EncodingKind {
    pub fn label(self) -> &'static str {
        match self {
            EncodingKind::Utf8 => "utf-8",
            EncodingKind::Utf16Le => "utf-16le",
            EncodingKind::Utf16Be => "utf-16be",
            EncodingKind::Windows1252 => "windows-1252",
        }
    }

    pub fn encoding(self) -> &'static Encoding {
        match self {
            EncodingKind::Utf8 => UTF_8,
            EncodingKind::Utf16Le => UTF_16LE,
            EncodingKind::Utf16Be => UTF_16BE,
            EncodingKind::Windows1252 => WINDOWS_1252,
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        let normalized: String = label
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| *c != '_' && *c != ' ')
            .collect();
        match normalized.as_str() {
            "utf8" | "utf-8" => Some(EncodingKind::Utf8),
            "utf16" | "utf-16" | "utf16le" | "utf-16le" | "utf16-le" => {
                Some(EncodingKind::Utf16Le)
            }
            "utf16be" | "utf-16be" | "utf16-be" => Some(EncodingKind::Utf16Be),
            "windows1252" | "windows-1252" | "cp1252" | "1252" => {
                Some(EncodingKind::Windows1252)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodingReport {
    pub label: String,
    pub source: ChoiceSource,
    /// A byte-order mark was present (and was removed by the decoder).
    pub bom: bool,
    /// At least one malformed byte sequence was replaced with U+FFFD.
    pub replacement_errors: bool,
    /// Number of decoder chunks that contained a replacement. This is a
    /// conservative count, not a count of individual characters.
    pub replacement_chunks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingDetection {
    pub kind: EncodingKind,
    pub bom: bool,
}

/// Detect one of the v1 encodings from a bounded prefix.
pub fn detect(bytes: &[u8]) -> EncodingDetection {
    if bytes.is_empty() {
        return EncodingDetection {
            kind: EncodingKind::Utf8,
            bom: false,
        };
    }
    if let Some((encoding, _bom_len)) = Encoding::for_bom(bytes) {
        let kind = if encoding == UTF_8 {
            EncodingKind::Utf8
        } else if encoding == UTF_16LE {
            EncodingKind::Utf16Le
        } else if encoding == UTF_16BE {
            EncodingKind::Utf16Be
        } else {
            // v1 only recognises the encodings in its own list.
            EncodingKind::Windows1252
        };
        return EncodingDetection { kind, bom: true };
    }
    if let Some(kind) = sniff_utf16_without_bom(bytes) {
        return EncodingDetection { kind, bom: false };
    }
    if is_utf8_prefix(bytes) {
        return EncodingDetection {
            kind: EncodingKind::Utf8,
            bom: false,
        };
    }
    EncodingDetection {
        kind: EncodingKind::Windows1252,
        bom: false,
    }
}

/// UTF-8 is accepted when it is entirely valid, or when the only error is an
/// incomplete sequence at the end of the sample window.
fn is_utf8_prefix(bytes: &[u8]) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none() && error.valid_up_to() + 4 >= bytes.len(),
    }
}

fn sniff_utf16_without_bom(bytes: &[u8]) -> Option<EncodingKind> {
    let sample = &bytes[..bytes.len().min(4096)];
    if sample.len() < 8 {
        return None;
    }
    let mut even_nul = 0usize;
    let mut odd_nul = 0usize;
    for (index, byte) in sample.iter().enumerate() {
        if *byte == 0 {
            if index % 2 == 0 {
                even_nul += 1;
            } else {
                odd_nul += 1;
            }
        }
    }
    let half = sample.len() / 2;
    // Latin text in UTF-16 has NULs in one byte of most code units. Require a
    // strong signal so binary or UTF-8 text is not misread as UTF-16.
    if odd_nul * 8 >= half * 3 && even_nul * 32 <= half {
        Some(EncodingKind::Utf16Le)
    } else if even_nul * 8 >= half * 3 && odd_nul * 32 <= half {
        Some(EncodingKind::Utf16Be)
    } else {
        None
    }
}

/// Decode a bounded sample for detection/inference. Invalid sequences become
/// U+FFFD; callers are told whether replacements occurred.
pub fn decode_sample(bytes: &[u8], kind: EncodingKind) -> (String, bool) {
    let (text, _, had_errors) = kind.encoding().decode(bytes);
    (text.into_owned(), had_errors)
}

/// Shared counters filled by a [`DecodeReader`].
#[derive(Debug, Clone)]
pub struct DecodeStats {
    source_read: Arc<AtomicU64>,
    replacement_chunks: Arc<AtomicU64>,
    /// `(decoded UTF-8 offset, source bytes consumed)` boundaries. Used to
    /// estimate how much of the source a record covered when a scan stops
    /// before EOF; interpolation is exact for ASCII/UTF-8 and approximate for
    /// multi-byte encodings.
    boundaries: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl Default for DecodeStats {
    fn default() -> Self {
        Self {
            source_read: Arc::new(AtomicU64::new(0)),
            replacement_chunks: Arc::new(AtomicU64::new(0)),
            boundaries: Arc::new(Mutex::new(vec![(0, 0)])),
        }
    }
}

impl DecodeStats {
    pub fn source_read(&self) -> u64 {
        self.source_read.load(Ordering::Relaxed)
    }

    pub fn replacement_chunks(&self) -> u64 {
        self.replacement_chunks.load(Ordering::Relaxed)
    }

    fn record_boundary(&self, decoded: u64, source: u64) {
        if let Ok(mut boundaries) = self.boundaries.lock() {
            if boundaries.last().map(|(d, _)| *d) != Some(decoded) || source > 0 {
                boundaries.push((decoded, source));
            }
        }
    }

    /// Estimate the source offset corresponding to a decoded byte offset.
    pub fn source_offset(&self, decoded: u64) -> u64 {
        let source_read = self.source_read();
        let boundaries = match self.boundaries.lock() {
            Ok(boundaries) => boundaries,
            Err(_) => return source_read,
        };
        if boundaries.len() < 2 {
            return source_read;
        }
        let index = boundaries
            .partition_point(|(offset, _)| *offset <= decoded)
            .saturating_sub(1);
        let (decoded_start, source_start) = boundaries[index];
        if let Some((decoded_end, source_end)) = boundaries.get(index + 1).copied() {
            if decoded_end > decoded_start {
                let span = decoded_end - decoded_start;
                let delta = decoded.saturating_sub(decoded_start).min(span);
                return (source_start
                    + (delta as u128 * (source_end - source_start) as u128 / span as u128) as u64)
                    .min(source_read);
            }
            source_end.min(source_read)
        } else {
            source_read
        }
    }
}

/// A `Read` adapter that decodes one of the supported encodings to UTF-8 and
/// reports how many source bytes have been read.
pub struct DecodeReader<R: Read> {
    inner: BufReader<R>,
    decoder: Decoder,
    input: Vec<u8>,
    input_pos: usize,
    output: Vec<u8>,
    output_pos: usize,
    eof: bool,
    flushed: bool,
    finished: bool,
    stats: DecodeStats,
    decoded_total: u64,
    buffer_size: usize,
}

impl<R: Read> DecodeReader<R> {
    pub fn new(reader: R, kind: EncodingKind, stats: DecodeStats) -> Self {
        Self {
            inner: BufReader::with_capacity(64 * 1024, reader),
            decoder: kind.encoding().new_decoder_with_bom_removal(),
            input: Vec::new(),
            input_pos: 0,
            output: Vec::new(),
            output_pos: 0,
            eof: false,
            flushed: false,
            finished: false,
            stats,
            decoded_total: 0,
            buffer_size: 64 * 1024,
        }
    }

    pub fn stats(&self) -> DecodeStats {
        self.stats.clone()
    }

    fn fill_output(&mut self) -> io::Result<()> {
        self.output.clear();
        self.output_pos = 0;
        while self.output.is_empty() && !self.finished {
            if self.input_pos == self.input.len() {
                if self.eof {
                    if self.flushed {
                        self.finished = true;
                        break;
                    }
                    self.flushed = true;
                } else {
                    self.input.clear();
                    self.input.resize(self.buffer_size, 0);
                    let read = self.inner.read(&mut self.input)?;
                    self.input.truncate(read);
                    self.input_pos = 0;
                    if read == 0 {
                        self.eof = true;
                        continue;
                    }
                    self.stats
                        .source_read
                        .fetch_add(read as u64, Ordering::Relaxed);
                }
            }

            let last = self.eof && self.input_pos == self.input.len();
            let mut scratch = vec![0u8; self.buffer_size];
            let (result, read, written, had_errors) = self.decoder.decode_to_utf8(
                &self.input[self.input_pos..],
                &mut scratch,
                last,
            );
            self.input_pos += read;
            if written > 0 {
                self.output.extend_from_slice(&scratch[..written]);
                self.decoded_total += written as u64;
            }
            let pending = self.input.len().saturating_sub(self.input_pos) as u64;
            let source_consumed = self.stats.source_read().saturating_sub(pending);
            self.stats
                .record_boundary(self.decoded_total, source_consumed);
            if had_errors {
                self.stats
                    .replacement_chunks
                    .fetch_add(1, Ordering::Relaxed);
            }
            match result {
                CoderResult::InputEmpty => {
                    if last && written == 0 {
                        self.finished = true;
                    }
                }
                CoderResult::OutputFull => {
                    // The next iteration starts with a fresh scratch buffer.
                }
            }
        }
        Ok(())
    }
}

impl<R: Read> Read for DecodeReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.output_pos == self.output.len() {
            if self.finished {
                return Ok(0);
            }
            self.fill_output()?;
        }
        let available = self.output.len() - self.output_pos;
        let take = available.min(buf.len());
        buf[..take].copy_from_slice(&self.output[self.output_pos..self.output_pos + take]);
        self.output_pos += take;
        Ok(take)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn detects_boms_and_encodings() {
        assert_eq!(detect(b"a,b\n1,2\n").kind, EncodingKind::Utf8);
        assert_eq!(detect(b"\xef\xbb\xbfa,b\n").kind, EncodingKind::Utf8);
        assert!(detect(b"\xef\xbb\xbfa,b\n").bom);
        assert_eq!(detect(b"\xff\xfea\0,\0b\0").kind, EncodingKind::Utf16Le);
        assert_eq!(detect(b"\xfe\xff\0a\0,\0b").kind, EncodingKind::Utf16Be);
        assert_eq!(detect(b"caf\xe9,1\n").kind, EncodingKind::Windows1252);
        assert_eq!(detect(b"").kind, EncodingKind::Utf8);
    }

    #[test]
    fn detects_utf16_without_bom() {
        let le: Vec<u8> = "a,b,c\n1,2,3\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(detect(&le).kind, EncodingKind::Utf16Le);
        let be: Vec<u8> = "a,b,c\n1,2,3\n".encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        assert_eq!(detect(&be).kind, EncodingKind::Utf16Be);
    }

    #[test]
    fn parses_known_labels() {
        assert_eq!(EncodingKind::parse("UTF-8"), Some(EncodingKind::Utf8));
        assert_eq!(EncodingKind::parse("cp1252"), Some(EncodingKind::Windows1252));
        assert_eq!(EncodingKind::parse("utf-16"), Some(EncodingKind::Utf16Le));
        assert_eq!(EncodingKind::parse("shift_jis"), None);
    }

    #[test]
    fn decodes_windows_1252_and_counts_replacements() {
        let stats = DecodeStats::default();
        let mut reader = DecodeReader::new(&b"caf\xe9\n"[..], EncodingKind::Windows1252, stats.clone());
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "caf\u{e9}\n");
        assert_eq!(stats.source_read(), 5);
        assert_eq!(stats.replacement_chunks(), 0);
    }

    #[test]
    fn decodes_utf16_bom_and_reports_source_bytes() {
        let mut bytes = vec![0xff, 0xfe];
        for unit in "a,b\n1,2\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let stats = DecodeStats::default();
        let mut reader = DecodeReader::new(&bytes[..], EncodingKind::Utf16Le, stats.clone());
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "a,b\n1,2\n");
        assert_eq!(stats.source_read(), bytes.len() as u64);
    }

    #[test]
    fn malformed_utf8_is_replaced_not_rejected() {
        let stats = DecodeStats::default();
        let mut reader = DecodeReader::new(&b"a\xffb\n"[..], EncodingKind::Utf8, stats.clone());
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert!(out.contains('\u{fffd}'));
        assert!(stats.replacement_chunks() > 0);
    }
}
