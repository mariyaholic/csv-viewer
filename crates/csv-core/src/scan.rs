// SPDX-License-Identifier: Apache-2.0

//! Incremental scan engine.
//!
//! The engine reads a bounded head for detection, then streams the rest
//! through a decoder and the `csv` crate. It never stores more than the
//! preview rows and bounded aggregates, and it stops on a record/byte/field
//! limit, cancellation, timeout or a malformed record. Caps, cancellation and
//! the deadline also hold inside one record, not only between records. A
//! partial result has no total.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use csv::{ReaderBuilder, StringRecord};

use crate::dialect::{
    detect as detect_dialect, detect_header, read_sample_records, Dialect, DialectChoice,
    DialectReport,
};
use crate::encoding::{
    decode_sample, detect as detect_encoding, ChoiceSource, DecodeReader, DecodeStats,
    EncodingReport, SAMPLE_BYTES,
};
use crate::market::MarketAccumulator;
use crate::model::{
    ColumnProfile, DateRangeReport, DateRangeRequest, DateRangeStatus, HeaderChoice, HeaderReport,
    IncompleteReason, InputInfo, PreviewRow, ScanCoverage, ScanError, ScanProgress, ScanReport,
    ScanRequest, TypeCounts,
};
use crate::types::{
    boolean_spelling, classify_cell, digit_count, has_embedded_newline, has_leading_zero,
    is_ambiguous_date, is_formula_like, is_link_like, parse_date, CellKind, CivilDate,
};
use crate::warnings::{WarningCode, WarningCollector};

/// Desktop row window. Source row indices exclude the detected header.
#[derive(Debug)]
pub struct BrowsePage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub source_rows: Vec<u64>,
    pub next: Option<u64>,
    pub next_byte: Option<u64>,
    pub scanned_to: u64,
    pub notes: String,
}
/// Streaming, bounded row window without retaining the prefix.
pub fn browse_page<R: Read + io::Seek>(
    source: R,
    offset: u64,
    limit: usize,
    query: &str,
) -> Result<BrowsePage, ScanError> {
    browse_page_at(source, offset, limit, query, None)
}
/// Resume a UTF-8 window at a previously emitted record boundary. Callers must
/// bind checkpoints to unchanged source metadata. Other encodings replay the
/// prefix; decoded UTF-8 positions must never be treated as source byte offsets.
pub fn browse_page_at<R: Read + io::Seek>(
    mut source: R,
    offset: u64,
    limit: usize,
    query: &str,
    byte: Option<u64>,
) -> Result<BrowsePage, ScanError> {
    if limit == 0 || limit > 200 || query.len() > 1024 {
        return Err(ScanError::InvalidRequest(
            "Invalid page size or search length".into(),
        ));
    }
    source.seek(io::SeekFrom::Start(0))?;
    let mut head = Vec::new();
    source
        .by_ref()
        .take(SAMPLE_BYTES as u64)
        .read_to_end(&mut head)?;
    let encoding = detect_encoding(&head).kind;
    let (sample, _) = decode_sample(&head, encoding);
    let dialect = detect_dialect(&sample).dialect;
    let sample = read_sample_records(&sample, &dialect, 50);
    let header = detect_header(&sample);
    let utf8 = encoding == crate::encoding::EncodingKind::Utf8;
    if byte.is_some() && !utf8 {
        return Err(ScanError::InvalidRequest(
            "Byte checkpoints require UTF-8".into(),
        ));
    }
    let columns = (0..sample.iter().map(|r| r.len()).max().unwrap_or(0).min(64))
        .map(|i| {
            if header {
                sample
                    .first()
                    .and_then(|r| r.get(i))
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("Column {}", i + 1))
            } else {
                format!("Column {}", i + 1)
            }
        })
        .collect();
    let indexed = byte.is_some();
    let bom = if utf8 && head.starts_with(b"\xef\xbb\xbf") {
        3
    } else {
        0
    };
    if byte.is_none() {
        head.drain(..bom);
    }
    let origin = byte.unwrap_or(bom as u64);
    let raw: Box<dyn Read + '_> = if let Some(byte) = byte {
        source.seek(io::SeekFrom::Start(byte))?;
        // Prime the CSV BOM recognizer using a discarded record. A real U+FEFF
        // at the beginning of a later data record must not be stripped as a BOM.
        Box::new(io::Cursor::new(b"X\n").chain(source))
    } else {
        Box::new(io::Cursor::new(head).chain(source))
    };
    let decoded: Box<dyn Read + '_> = if utf8 {
        raw
    } else {
        Box::new(DecodeReader::new(raw, encoding, DecodeStats::default()))
    };
    let malformed = Arc::new(AtomicBool::new(false));
    let cancelled = AtomicBool::new(false);
    let halt = Arc::new(AtomicU8::new(HALT_NONE));
    let guarded = QuoteGuard::new(
        decoded,
        &dialect,
        malformed.clone(),
        ReaderBounds {
            halt: halt.clone(),
            cancel: &cancelled,
            started: Instant::now(),
            deadline: Some(Duration::from_secs(20)),
            field_budget: Some(16 * 1024),
            record_budget: Some(1024 * 1024),
        },
    );
    let mut reader = ReaderBuilder::new()
        .delimiter(dialect.delimiter as u8)
        .quote(dialect.quote as u8)
        .double_quote(dialect.double_quote)
        .escape(dialect.escape.map(|c| c as u8))
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(1024)
        .from_reader(guarded);
    let mut record = StringRecord::new();
    if indexed {
        reader
            .read_record(&mut record)
            .map_err(|e| ScanError::InvalidRequest(e.to_string()))?;
    }
    let mut page = BrowsePage {
        columns,
        rows: Vec::new(),
        source_rows: Vec::new(),
        next: None,
        next_byte: None,
        scanned_to: offset,
        notes: format!(
            "Encoding: {}; delimiter: {:?}; automatic header: {}; first 64 columns. {}",
            encoding.label(),
            dialect.delimiter,
            header,
            if utf8 {
                "UTF-8 record checkpoints."
            } else {
                "Decoded prefix replay; later pages may reach the 20-second budget."
            }
        ),
    };
    let needle = query.to_lowercase();
    let mut cursor = if indexed { offset } else { 0 };
    let mut retained_bytes = 0usize;
    let mut first = !indexed;
    let mut next_byte = if utf8 { Some(origin) } else { None };
    loop {
        let start_byte = next_byte;
        match reader.read_record(&mut record) {
            Ok(false) => {
                if malformed.load(Ordering::Relaxed) {
                    return Err(ScanError::InvalidRequest("Unclosed quoted record".into()));
                }
                page.scanned_to = cursor;
                return Ok(page);
            }
            Ok(true) => {}
            Err(_) if halt.load(Ordering::Relaxed) == HALT_TIME_LIMIT && cursor > offset => {
                page.next = Some(cursor);
                page.next_byte = start_byte;
                page.scanned_to = cursor;
                return Ok(page);
            }
            Err(e) => return Err(ScanError::InvalidRequest(e.to_string())),
        }
        if malformed.load(Ordering::Relaxed) {
            return Err(ScanError::InvalidRequest("Unclosed quoted record".into()));
        }
        next_byte = if utf8 {
            Some(origin + reader.position().byte() - if indexed { 2 } else { 0 })
        } else {
            None
        };
        if first {
            first = false;
            if header {
                continue;
            }
        }
        while page.columns.len() < record.len().min(64) {
            page.columns
                .push(format!("Column {}", page.columns.len() + 1));
        }
        let row = cursor;
        cursor += 1;
        if row >= offset
            && (needle.is_empty()
                || record
                    .iter()
                    .take(64)
                    .any(|cell| cell.to_lowercase().contains(&needle)))
        {
            let cost: usize = record.iter().take(64).map(str::len).sum();
            // Worst-case control escaping expands sixfold, still below the
            // shell's 8 MiB display budget and 16 MiB framed transport cap.
            if retained_bytes + cost > 1024 * 1024 {
                page.next = Some(row);
                page.next_byte = start_byte;
                page.scanned_to = row;
                return Ok(page);
            }
            retained_bytes += cost;
            page.rows
                .push(record.iter().take(64).map(str::to_owned).collect());
            page.source_rows.push(row);
        }
        if page.rows.len() >= limit {
            page.next = Some(cursor);
            page.next_byte = next_byte;
            page.scanned_to = cursor;
            return Ok(page);
        }
    }
}

/// Scan a file read-only. The caller supplies a length hint so coverage can
/// display a denominator.
pub fn scan_file(path: &Path, request: &ScanRequest) -> Result<ScanReport, ScanError> {
    scan_file_with_progress(path, request, &AtomicBool::new(false), &mut |_| {})
}

pub fn scan_file_with_progress(
    path: &Path,
    request: &ScanRequest,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanReport, ScanError> {
    let metadata = std::fs::metadata(path)?;
    scan_file_with_info(
        path,
        InputInfo::known(metadata.len()),
        request,
        cancel,
        progress,
    )
}

/// Like [`scan_file_with_progress`], but lets the caller state whether the
/// spooled source was truncated and how many bytes it contains.
pub fn scan_file_with_info(
    path: &Path,
    info: InputInfo,
    request: &ScanRequest,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanReport, ScanError> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(ScanError::InvalidRequest(
            "source is not a regular file".to_string(),
        ));
    }
    if let Some(total) = info.total_bytes {
        if total != metadata.len() {
            return Err(ScanError::InvalidRequest(format!(
                "declared source length {total} does not match file length {}",
                metadata.len()
            )));
        }
    }
    let file = File::open(path)?;
    scan_with_progress(file, info, request, cancel, progress)
}

pub fn scan<R: Read>(
    reader: R,
    info: InputInfo,
    request: &ScanRequest,
) -> Result<ScanReport, ScanError> {
    scan_with_progress(reader, info, request, &AtomicBool::new(false), &mut |_| {})
}

pub fn scan_with_progress<R: Read>(
    mut reader: R,
    info: InputInfo,
    request: &ScanRequest,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanReport, ScanError> {
    request.validate()?;
    let started = Instant::now();
    let deadline = request
        .limits
        .deadline_ms
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis);

    // A bounded head drives encoding, dialect and header inference. It is
    // replayed into the streaming parse so no bytes are skipped.
    let mut head = Vec::with_capacity(SAMPLE_BYTES);
    reader
        .by_ref()
        .take(SAMPLE_BYTES as u64)
        .read_to_end(&mut head)?;

    let detection = detect_encoding(&head);
    let (encoding_kind, encoding_source) = match request.encoding {
        crate::model::EncodingChoice::Auto => (detection.kind, ChoiceSource::Auto),
        crate::model::EncodingChoice::Explicit(kind) => (kind, ChoiceSource::Override),
    };
    let (sample_text, _sample_replacements) = decode_sample(&head, encoding_kind);

    let (dialect, dialect_source, dialect_confidence) = match request.dialect {
        DialectChoice::Auto => {
            let detected = detect_dialect(&sample_text);
            (detected.dialect, ChoiceSource::Auto, detected.confidence)
        }
        DialectChoice::Explicit(dialect) => (dialect, ChoiceSource::Override, 100),
    };
    dialect.validate().map_err(ScanError::InvalidRequest)?;

    let sample_rows = read_sample_records(&sample_text, &dialect, 50);
    // The csv crate's line counter is one low for CRLF files (observed for
    // both record and reader positions). Preview line numbers are display-only;
    // adjust only for records after the first one.
    let crlf_line_adjust = if sample_text.contains("\r\n") { 1 } else { 0 };
    let header_present = match request.header {
        HeaderChoice::Auto => detect_header(&sample_rows),
        HeaderChoice::Present => true,
        HeaderChoice::Absent => false,
    };
    let header_source = match request.header {
        HeaderChoice::Auto => ChoiceSource::Auto,
        HeaderChoice::Present | HeaderChoice::Absent => ChoiceSource::Override,
    };

    let stats = DecodeStats::default();
    let source = io::Cursor::new(head).chain(reader);
    let decoded = DecodeReader::new(source, encoding_kind, stats.clone());
    let malformed = Arc::new(AtomicBool::new(false));
    let halt = Arc::new(AtomicU8::new(HALT_NONE));
    // A field may only grow to `max_field_chars * 4` bytes before the scan
    // stops: caps, cancellation and deadlines must hold inside one record,
    // not only between records.
    let bounds = ReaderBounds {
        halt: halt.clone(),
        cancel,
        started,
        deadline,
        record_budget: None,
        field_budget: Some(
            u64::try_from(request.limits.max_field_chars)
                .unwrap_or(u64::MAX)
                .saturating_mul(4),
        ),
    };
    let guarded = QuoteGuard::new(decoded, &dialect, malformed.clone(), bounds);
    let mut csv_reader = build_reader(guarded, &dialect);

    let mut state = ScanState::new(request, progress, info.total_bytes);
    let mut record = StringRecord::new();
    let mut end = ScanEnd {
        complete: false,
        reason: None,
        parse_error: None,
    };

    // Read the first record separately: it may be a header or the first data
    // row that establishes the column set.
    let first_line = csv_reader.position().line();
    match csv_reader.read_record(&mut record) {
        Ok(true) => {
            let cells: Vec<String> = record.iter().map(str::to_string).collect();
            state.rows += 1;
            if header_present {
                state.set_header(&cells);
            } else {
                state.ensure_columns(cells.len(), 0);
                state.data_records += 1;
                state.process_row(&cells, 1, first_line);
            }
        }
        Ok(false) => {
            end.complete = true;
        }
        Err(error) => {
            end.reason = Some(IncompleteReason::ParseError);
            end.parse_error = Some(error.to_string());
        }
    }
    // A halt while reading the first record already ends the scan; the loop
    // below would otherwise not consult the guard again.
    apply_reader_halt(&halt, &mut end);
    state.update_bytes(stats.source_offset(csv_reader.position().byte()));

    while end.parse_error.is_none() && !end.complete {
        if apply_reader_halt(&halt, &mut end) {
            break;
        }
        if cancel.load(Ordering::Relaxed) {
            end.reason = Some(IncompleteReason::Cancelled);
            break;
        }
        if let Some(deadline) = deadline {
            if started.elapsed() >= deadline {
                end.reason = Some(IncompleteReason::TimeLimit);
                break;
            }
        }
        if let Some(max) = request.limits.max_records {
            if state.data_records >= max {
                end.reason = Some(IncompleteReason::RecordLimit);
                break;
            }
        }
        if let Some(max) = request.limits.max_bytes {
            if stats.source_read() >= max {
                end.reason = Some(IncompleteReason::ByteLimit);
                break;
            }
        }

        let line = csv_reader.position().line() + crlf_line_adjust;
        match csv_reader.read_record(&mut record) {
            Ok(true) => {
                let cells: Vec<String> = record.iter().map(str::to_string).collect();
                state.rows += 1;
                state.data_records += 1;
                state.process_row(&cells, state.data_records, line);
                state.update_bytes(stats.source_offset(csv_reader.position().byte()));
                if state.data_records % 1_000 == 0 {
                    state.emit_progress(stats.source_read());
                }
            }
            Ok(false) => {
                state.update_bytes(stats.source_read());
                end.complete = true;
            }
            Err(error) => {
                end.reason = Some(IncompleteReason::ParseError);
                end.parse_error = Some(error.to_string());
            }
        }
        // Refine the error: the guard distinguishes a cancellation or
        // deadline reached inside a record from a plain parse error.
        apply_reader_halt(&halt, &mut end);
    }

    if malformed.load(Ordering::Relaxed) && end.reason.is_none() {
        end.complete = false;
        end.reason = Some(IncompleteReason::ParseError);
        end.parse_error = Some("unterminated quoted field at end of source".to_string());
    }

    let final_bytes = if end.complete && !info.truncated && !info.preview_only {
        info.total_bytes.unwrap_or_else(|| stats.source_read())
    } else {
        state
            .bytes
            .max(stats.source_offset(csv_reader.position().byte()))
    };
    state.update_bytes(final_bytes);
    state.emit_progress(final_bytes);

    if end.complete {
        if info.preview_only {
            end.complete = false;
            end.reason = Some(IncompleteReason::PreviewOnly);
        } else if info.truncated {
            end.complete = false;
            end.reason = Some(IncompleteReason::SourceTruncated);
        }
    }
    if !end.complete && end.reason.is_none() {
        end.reason = Some(IncompleteReason::SourceTruncated);
    }

    let encoding_report = EncodingReport {
        label: encoding_kind.label().to_string(),
        source: encoding_source,
        bom: detection.bom,
        replacement_errors: stats.replacement_chunks() > 0,
        replacement_chunks: stats.replacement_chunks(),
    };
    let dialect_report = DialectReport {
        delimiter: dialect.delimiter,
        quote: dialect.quote,
        source: dialect_source,
        confidence: dialect_confidence,
    };
    let report = state.finish(
        info,
        end,
        encoding_report,
        dialect_report,
        header_present,
        header_source,
        final_bytes,
    );
    if let Err(message) = report.validate_invariants() {
        return Err(ScanError::InvalidRequest(message));
    }
    Ok(report)
}

/// Tracks whether the decoded byte stream ends inside a quoted field. The
/// `csv` crate is deliberately lenient at EOF; a preview must not call an
/// unterminated quote a complete record.
/// Codes a halt that must stop the scan from inside the reader. 0 keeps
/// reading; the scan loop maps the others to report reasons.
const HALT_NONE: u8 = 0;
const HALT_FIELD_BUDGET: u8 = 1;
const HALT_CANCELLED: u8 = 2;
const HALT_TIME_LIMIT: u8 = 3;

/// Maps a mid-field halt inside the reader to the report's end state. A
/// budget stop is a parse error (the record is unfinished); a cancellation or
/// deadline is not. Returns whether a halt was pending.
fn apply_reader_halt(halt: &AtomicU8, end: &mut ScanEnd) -> bool {
    match halt.swap(HALT_NONE, Ordering::Relaxed) {
        HALT_NONE => false,
        HALT_FIELD_BUDGET => {
            end.complete = false;
            end.reason = Some(IncompleteReason::ParseError);
            end.parse_error = Some("a single field exceeded the per-field byte budget".to_string());
            true
        }
        HALT_CANCELLED => {
            end.complete = false;
            end.reason = Some(IncompleteReason::Cancelled);
            end.parse_error = None;
            true
        }
        HALT_TIME_LIMIT => {
            end.complete = false;
            end.reason = Some(IncompleteReason::TimeLimit);
            end.parse_error = None;
            true
        }
        _ => false,
    }
}

/// Bounds enforced by the reader in addition to the quote state: a shared
/// halt slot with the scan loop, the cancellation flag and deadline, and the
/// hard byte budget per field (`None` disables it).
struct ReaderBounds<'a> {
    halt: Arc<AtomicU8>,
    cancel: &'a AtomicBool,
    started: Instant,
    deadline: Option<Duration>,
    field_budget: Option<u64>,
    record_budget: Option<u64>,
}

struct QuoteGuard<'a, R: Read> {
    inner: R,
    quote: u8,
    delimiter: u8,
    escape: Option<u8>,
    double_quote: bool,
    state: QuoteState,
    malformed: Arc<AtomicBool>,
    bounds: ReaderBounds<'a>,
    /// Bytes counted since the current field started.
    field_bytes: u64,
    record_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    FieldStart,
    Unquoted,
    Quoted,
    QuoteClosed,
    Escaped,
}

impl<'a, R: Read> QuoteGuard<'a, R> {
    fn new(
        inner: R,
        dialect: &Dialect,
        malformed: Arc<AtomicBool>,
        bounds: ReaderBounds<'a>,
    ) -> Self {
        Self {
            inner,
            quote: dialect.quote as u8,
            delimiter: dialect.delimiter as u8,
            escape: dialect.escape.map(|escape| escape as u8),
            double_quote: dialect.double_quote,
            state: QuoteState::FieldStart,
            malformed,
            bounds,
            field_bytes: 0,
            record_bytes: 0,
        }
    }

    fn feed(&mut self, byte: u8) {
        match self.state {
            QuoteState::FieldStart => {
                if byte == self.quote {
                    self.state = QuoteState::Quoted;
                } else if byte == self.delimiter || is_line_break(byte) {
                    self.state = QuoteState::FieldStart;
                } else {
                    self.state = QuoteState::Unquoted;
                }
            }
            QuoteState::Unquoted => {
                if byte == self.delimiter || is_line_break(byte) {
                    self.state = QuoteState::FieldStart;
                }
            }
            QuoteState::Quoted => {
                if Some(byte) == self.escape {
                    self.state = QuoteState::Escaped;
                } else if byte == self.quote {
                    self.state = QuoteState::QuoteClosed;
                }
            }
            QuoteState::QuoteClosed => {
                if self.double_quote && byte == self.quote {
                    self.state = QuoteState::Quoted;
                } else if byte == self.delimiter || is_line_break(byte) {
                    self.state = QuoteState::FieldStart;
                } else if byte == b' ' || byte == b'\t' {
                    // Tolerate whitespace after a closing quote.
                } else {
                    self.state = QuoteState::Quoted;
                }
            }
            QuoteState::Escaped => {
                self.state = QuoteState::Quoted;
            }
        }
    }
}

fn is_line_break(byte: u8) -> bool {
    byte == b'\n' || byte == b'\r'
}

impl<R: Read> Read for QuoteGuard<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // Memory, cancellation and deadline bounds must also hold inside a
        // single record, not only between records: one over-long or broken
        // field could otherwise buffer the rest of the source.
        if self.bounds.cancel.load(Ordering::Relaxed) {
            self.bounds.halt.store(HALT_CANCELLED, Ordering::Relaxed);
            return Err(io::Error::other("scan cancelled inside a record"));
        }
        if let Some(deadline) = self.bounds.deadline {
            if self.bounds.started.elapsed() >= deadline {
                self.bounds.halt.store(HALT_TIME_LIMIT, Ordering::Relaxed);
                return Err(io::Error::other("scan deadline reached inside a record"));
            }
        }
        let read = self.inner.read(buffer)?;
        if read == 0 {
            if matches!(self.state, QuoteState::Quoted | QuoteState::Escaped) {
                self.malformed.store(true, Ordering::Relaxed);
            }
            return Ok(0);
        }
        for &byte in &buffer[..read] {
            let state_before = self.state;
            self.feed(byte);
            self.record_bytes = self.record_bytes.saturating_add(1);
            if self
                .bounds
                .record_budget
                .is_some_and(|cap| self.record_bytes > cap)
            {
                return Err(io::Error::other(
                    "record exceeds the desktop 1 MiB memory budget",
                ));
            }
            if is_line_break(byte) && self.state == QuoteState::FieldStart {
                self.record_bytes = 0;
            }
            match (state_before, self.state) {
                // A fresh field starts with this byte.
                (QuoteState::FieldStart, QuoteState::Unquoted | QuoteState::Quoted) => {
                    self.field_bytes = 1;
                }
                // A field or record boundary consumed this byte; the next
                // field starts with the following byte.
                (_, QuoteState::FieldStart) => {
                    self.field_bytes = 0;
                }
                // Data continues inside the current field.
                _ => {
                    self.field_bytes = self.field_bytes.saturating_add(1);
                }
            }
            if let Some(budget) = self.bounds.field_budget {
                if self.field_bytes > budget {
                    self.bounds.halt.store(HALT_FIELD_BUDGET, Ordering::Relaxed);
                    return Err(io::Error::other(format!(
                        "a field exceeded the per-field budget of {budget} bytes"
                    )));
                }
            }
        }
        Ok(read)
    }
}

fn build_reader<R: Read>(reader: R, dialect: &Dialect) -> csv::Reader<R> {
    let mut builder = ReaderBuilder::new();
    builder
        .delimiter(dialect.delimiter as u8)
        .quote(dialect.quote as u8)
        .double_quote(dialect.double_quote)
        .flexible(true)
        .has_headers(false)
        .buffer_capacity(64 * 1024);
    if let Some(escape) = dialect.escape {
        builder.escape(Some(escape as u8));
    }
    builder.from_reader(reader)
}

struct ScanEnd {
    complete: bool,
    reason: Option<IncompleteReason>,
    parse_error: Option<String>,
}

#[derive(Default)]
struct ColumnExtras {
    bool_spellings: BTreeMap<String, u64>,
}

struct DateAccumulator {
    request: DateRangeRequest,
    parsed: u64,
    invalid: u64,
    missing: u64,
    min: Option<CivilDate>,
    max: Option<CivilDate>,
}

struct ScanState<'a> {
    request: &'a ScanRequest,
    warnings: WarningCollector,
    columns: Vec<ColumnProfile>,
    extras: Vec<ColumnExtras>,
    preview: Vec<PreviewRow>,
    dates: Vec<DateAccumulator>,
    market: Option<MarketAccumulator>,
    header_raw: Vec<String>,
    preview_chars: usize,
    data_records: u64,
    rows: u64,
    bytes: u64,
    total_bytes: Option<u64>,
    progress: &'a mut dyn FnMut(ScanProgress),
}

impl<'a> ScanState<'a> {
    fn new(
        request: &'a ScanRequest,
        progress: &'a mut dyn FnMut(ScanProgress),
        total_bytes: Option<u64>,
    ) -> Self {
        let dates = request
            .date_ranges
            .iter()
            .map(|date_request| DateAccumulator {
                request: date_request.clone(),
                parsed: 0,
                invalid: 0,
                missing: 0,
                min: None,
                max: None,
            })
            .collect();
        Self {
            request,
            warnings: WarningCollector::new(),
            columns: Vec::new(),
            extras: Vec::new(),
            preview: Vec::new(),
            dates,
            market: None,
            header_raw: Vec::new(),
            preview_chars: 0,
            data_records: 0,
            rows: 0,
            bytes: 0,
            total_bytes,
            progress,
        }
    }

    fn update_bytes(&mut self, bytes: u64) {
        self.bytes = bytes;
    }

    fn emit_progress(&mut self, bytes: u64) {
        (self.progress)(ScanProgress {
            data_records: self.data_records,
            bytes,
            total_bytes: self.total_bytes,
        });
    }

    fn set_header(&mut self, raw: &[String]) {
        let raw: Vec<String> = raw.iter().map(|cell| truncate_chars(cell, 200).0).collect();
        self.header_raw = raw.clone();
        let names = header_names(&raw, self.request.limits.max_columns, &mut self.warnings);
        self.install_columns(names);
    }

    fn install_columns(&mut self, names: Vec<String>) {
        self.columns = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| ColumnProfile {
                index,
                name,
                inferred: CellKind::Empty,
                counts: TypeCounts::default(),
                max_chars: 0,
                samples: Vec::new(),
            })
            .collect();
        self.extras = (0..self.columns.len())
            .map(|_| ColumnExtras::default())
            .collect();
        self.ensure_market();
    }

    fn ensure_columns(&mut self, count: usize, record: u64) {
        let target = count.min(self.request.limits.max_columns);
        if count > self.request.limits.max_columns {
            self.warnings.add(
                None,
                WarningCode::ColumnLimitReached,
                record,
                &count.to_string(),
            );
        }
        while self.columns.len() < target {
            let index = self.columns.len();
            let name = format!("column_{}", index + 1);
            self.columns.push(ColumnProfile {
                index,
                name,
                inferred: CellKind::Empty,
                counts: TypeCounts::default(),
                max_chars: 0,
                samples: Vec::new(),
            });
            self.extras.push(ColumnExtras::default());
        }
        self.ensure_market();
    }

    fn ensure_market(&mut self) {
        if self.market.is_some() || self.columns.is_empty() {
            return;
        }
        if let Some(request) = &self.request.market {
            let names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();
            self.market = Some(MarketAccumulator::new(request.clone(), &names));
        }
    }

    fn process_row(&mut self, cells: &[String], record: u64, line: u64) {
        let expected = self.columns.len();
        if expected > 0 && cells.len() != expected {
            self.warnings.add(
                None,
                WarningCode::RaggedRow,
                record,
                &format!("{} fields, expected {}", cells.len(), expected),
            );
        }
        self.ensure_columns(cells.len(), record);
        let column_count = self.columns.len();

        let mut preview_cells = Vec::with_capacity(column_count);
        let mut preview_truncated = false;
        let max_preview_chars = self.request.limits.max_preview_chars;
        let mut preview_open = self.preview.len() < self.request.preview_rows
            && self.preview_chars < max_preview_chars;
        let started_preview = preview_open;

        for index in 0..column_count {
            let raw = cells.get(index).map(String::as_str).unwrap_or("");
            let kind = classify_cell(raw);
            let chars = raw.chars().count();
            {
                let profile = &mut self.columns[index];
                profile.counts.add(kind);
                profile.max_chars = profile.max_chars.max(chars);
                if !raw.trim().is_empty() && profile.samples.len() < 3 {
                    let (sample, _) = truncate_chars(raw, 200);
                    profile.samples.push(sample);
                }
            }

            if has_leading_zero(raw) {
                self.warnings
                    .add(Some(index), WarningCode::LeadingZeros, record, raw);
            }
            if is_ambiguous_date(raw) {
                self.warnings
                    .add(Some(index), WarningCode::AmbiguousDate, record, raw);
            }
            if kind == CellKind::Integer && digit_count(raw) > 15 {
                self.warnings
                    .add(Some(index), WarningCode::LongNumber, record, raw);
            }
            if is_formula_like(raw) {
                self.warnings
                    .add(Some(index), WarningCode::FormulaLikeText, record, raw);
            }
            if is_link_like(raw) {
                self.warnings
                    .add(Some(index), WarningCode::LinkLikeText, record, raw);
            }
            if has_embedded_newline(raw) {
                self.warnings
                    .add(Some(index), WarningCode::QuotedMultiline, record, raw);
            }

            if let Some(spelling) = boolean_spelling(raw) {
                *self.extras[index]
                    .bool_spellings
                    .entry(spelling.to_string())
                    .or_insert(0) += 1;
            } else {
                let trimmed = raw.trim();
                if trimmed == "0" || trimmed == "1" {
                    *self.extras[index]
                        .bool_spellings
                        .entry(trimmed.to_string())
                        .or_insert(0) += 1;
                }
            }

            for date in &mut self.dates {
                if date.request.column != index {
                    continue;
                }
                if raw.trim().is_empty() {
                    date.missing += 1;
                } else if let Some(parsed) = parse_date(raw, date.request.format) {
                    date.parsed += 1;
                    date.min = Some(date.min.map_or(parsed, |current| current.min(parsed)));
                    date.max = Some(date.max.map_or(parsed, |current| current.max(parsed)));
                } else {
                    date.invalid += 1;
                }
            }

            let field_too_long = chars > self.request.limits.max_field_chars;
            if field_too_long {
                self.warnings
                    .add(Some(index), WarningCode::LongField, record, "");
            }
            if preview_open {
                let (display, _) = truncate_chars(raw, self.request.limits.max_field_chars);
                let cell_chars = display.chars().count();
                if self.preview_chars + cell_chars > max_preview_chars {
                    preview_truncated = true;
                    preview_open = false;
                } else {
                    self.preview_chars += cell_chars;
                    preview_cells.push(display);
                    preview_truncated |= field_too_long;
                }
            }
        }

        if started_preview {
            self.preview.push(PreviewRow {
                record,
                line,
                cells: preview_cells,
                cells_truncated: preview_truncated,
            });
        }

        if let Some(market) = &mut self.market {
            market.feed(cells);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        mut self,
        info: InputInfo,
        end: ScanEnd,
        encoding: EncodingReport,
        dialect: DialectReport,
        header_present: bool,
        header_source: ChoiceSource,
        final_bytes: u64,
    ) -> ScanReport {
        if encoding.replacement_errors {
            self.warnings.add_count(
                None,
                WarningCode::ReplacementErrors,
                encoding.replacement_chunks.max(1),
                0,
                "",
            );
        }

        for (index, profile) in self.columns.iter_mut().enumerate() {
            profile.inferred = profile.counts.inferred();
            let non_empty = profile.counts.non_empty();
            let numeric = profile.counts.integer + profile.counts.float;
            let other = non_empty.saturating_sub(numeric + profile.counts.text);
            let minority = profile.counts.text.min(numeric + other);
            if profile.counts.text > 0
                && (numeric + profile.counts.boolean + profile.counts.date) > 0
                && minority >= 2
                && minority * 20 >= non_empty
            {
                self.warnings.add(
                    Some(index),
                    WarningCode::MixedTypes,
                    0,
                    profile.inferred.as_str(),
                );
            }

            let extras = &self.extras[index];
            let has_word = extras
                .bool_spellings
                .keys()
                .any(|key| key != "0" && key != "1");
            if has_word && extras.bool_spellings.len() > 1 {
                let count: u64 = extras.bool_spellings.values().sum();
                let sample = extras
                    .bool_spellings
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                self.warnings.add_count(
                    Some(index),
                    WarningCode::BooleanSpellings,
                    count,
                    0,
                    &sample,
                );
            }
        }

        let complete = end.complete && !info.truncated && !info.preview_only;
        let incomplete_reason = if complete {
            None
        } else {
            Some(end.reason.unwrap_or(IncompleteReason::SourceTruncated))
        };
        let total_bytes = if complete {
            info.total_bytes.or(Some(final_bytes))
        } else {
            info.total_bytes
        };

        let date_ranges = self.date_reports(complete);
        let market = self.market.take().map(|market| market.finish(complete));
        let column_names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();
        let warnings = self.warnings.finish(&column_names);

        let total_records = if complete {
            Some(self.data_records)
        } else {
            None
        };

        ScanReport {
            complete,
            incomplete_reason,
            total_records,
            scanned: ScanCoverage {
                data_records: self.data_records,
                rows: self.rows,
                bytes: final_bytes,
                total_bytes,
                source_truncated: info.truncated,
            },
            encoding,
            dialect,
            header: HeaderReport {
                present: header_present && !self.header_raw.is_empty(),
                source: header_source,
                raw: self.header_raw,
                columns: column_names,
            },
            columns: self.columns,
            preview_rows: self.preview,
            warnings,
            date_ranges,
            market,
            parse_error: end.parse_error,
        }
    }

    fn date_reports(&self, complete: bool) -> Vec<DateRangeReport> {
        self.dates
            .iter()
            .map(|date| {
                let column_name = self
                    .columns
                    .get(date.request.column)
                    .map(|c| c.name.clone());
                if !complete {
                    return DateRangeReport {
                        column: date.request.column,
                        column_name,
                        format: date.request.format,
                        status: DateRangeStatus::Unavailable,
                        min: None,
                        max: None,
                        parsed: date.parsed,
                        invalid: date.invalid,
                        missing: date.missing,
                        reason: Some(
                            "a complete scan is required before a date range can be shown"
                                .to_string(),
                        ),
                    };
                }
                if column_name.is_none() {
                    return DateRangeReport {
                        column: date.request.column,
                        column_name,
                        format: date.request.format,
                        status: DateRangeStatus::Unavailable,
                        min: None,
                        max: None,
                        parsed: date.parsed,
                        invalid: date.invalid,
                        missing: date.missing,
                        reason: Some("selected column was not found".to_string()),
                    };
                }
                if date.parsed == 0 {
                    return DateRangeReport {
                        column: date.request.column,
                        column_name,
                        format: date.request.format,
                        status: DateRangeStatus::Unavailable,
                        min: None,
                        max: None,
                        parsed: 0,
                        invalid: date.invalid,
                        missing: date.missing,
                        reason: Some(
                            "no values matched the chosen date interpretation".to_string(),
                        ),
                    };
                }
                DateRangeReport {
                    column: date.request.column,
                    column_name,
                    format: date.request.format,
                    status: DateRangeStatus::Computed,
                    min: date.min.map(|date| date.to_iso()),
                    max: date.max.map(|date| date.to_iso()),
                    parsed: date.parsed,
                    invalid: date.invalid,
                    missing: date.missing,
                    reason: None,
                }
            })
            .collect()
    }
}

fn header_names(
    raw: &[String],
    max_columns: usize,
    warnings: &mut WarningCollector,
) -> Vec<String> {
    if raw.len() > max_columns {
        warnings.add(
            None,
            WarningCode::ColumnLimitReached,
            0,
            &raw.len().to_string(),
        );
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut names = Vec::new();
    for (index, value) in raw.iter().take(max_columns).enumerate() {
        let trimmed = value.trim();
        let mut name = if trimmed.is_empty() {
            warnings.add(Some(index), WarningCode::HeaderEmpty, 0, value);
            format!("column_{}", index + 1)
        } else {
            truncate_chars(trimmed, 200).0
        };
        let count = seen.entry(name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            warnings.add(Some(index), WarningCode::HeaderDuplicate, 0, value);
            name = format!("{name} ({count})");
        }
        names.push(name);
    }
    names
}

fn truncate_chars(value: &str, max: usize) -> (String, bool) {
    if value.chars().count() <= max {
        return (value.to_string(), false);
    }
    let mut result = String::new();
    for ch in value.chars().take(max) {
        result.push(ch);
    }
    (result, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DateRangeRequest;
    use crate::types::DateFormat;

    fn scan_text(text: &str, request: &ScanRequest) -> ScanReport {
        scan(
            io::Cursor::new(text.as_bytes().to_vec()),
            InputInfo::known(text.len() as u64),
            request,
        )
        .unwrap()
    }

    #[test]
    fn complete_scan_has_exact_total() {
        let report = scan_text("a,b\n1,2\n3,4\n", &ScanRequest::full());
        assert!(report.complete);
        assert_eq!(report.total_records, Some(2));
        assert_eq!(report.scanned.data_records, 2);
        assert_eq!(report.scanned.rows, 3);
        assert!(report.header.present);
        assert_eq!(report.columns.len(), 2);
    }

    #[test]
    fn record_limit_never_reports_a_total() {
        let mut request = ScanRequest::full();
        request.limits.max_records = Some(1);
        let report = scan_text("a,b\n1,2\n3,4\n", &request);
        assert!(!report.complete);
        assert_eq!(
            report.incomplete_reason,
            Some(IncompleteReason::RecordLimit)
        );
        assert_eq!(report.total_records, None);
        assert_eq!(report.scanned.data_records, 1);
        assert!(report.scanned.bytes > 0);
        assert!(
            report.scanned.bytes < 12,
            "partial coverage must not claim the whole source"
        );
    }

    #[test]
    fn preview_is_always_incomplete() {
        let report = scan(
            io::Cursor::new(b"a,b\n1,2\n".to_vec()),
            InputInfo::preview(Some(8)),
            &ScanRequest::preview(),
        )
        .unwrap();
        assert!(!report.complete);
        assert_eq!(
            report.incomplete_reason,
            Some(IncompleteReason::PreviewOnly)
        );
        assert_eq!(report.total_records, None);
    }

    #[test]
    fn quoted_multiline_record_is_preserved() {
        let report = scan_text("a,b\n1,\"two\nlines\"\n", &ScanRequest::full());
        assert_eq!(report.preview_rows[0].cells[1], "two\nlines");
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::QuotedMultiline));
    }

    #[test]
    fn date_range_requires_complete_scan() {
        let mut request = ScanRequest::full();
        request.date_ranges.push(DateRangeRequest {
            column: 0,
            format: DateFormat::Ymd,
        });
        let report = scan_text("date,value\n2024-01-02,1\n2024-01-01,2\n", &request);
        let range = &report.date_ranges[0];
        assert_eq!(range.status, DateRangeStatus::Computed);
        assert_eq!(range.min.as_deref(), Some("2024-01-01"));
        assert_eq!(range.max.as_deref(), Some("2024-01-02"));

        request.limits.max_records = Some(1);
        let report = scan_text("date,value\n2024-01-02,1\n2024-01-01,2\n", &request);
        assert_eq!(report.date_ranges[0].status, DateRangeStatus::Unavailable);
        assert!(report.date_ranges[0].min.is_none());
    }

    #[test]
    fn cancellation_is_a_partial_reason() {
        let cancel = AtomicBool::new(true);
        let report = scan_with_progress(
            io::Cursor::new(b"a,b\n1,2\n3,4\n".to_vec()),
            InputInfo::known(12),
            &ScanRequest::full(),
            &cancel,
            &mut |_| {},
        )
        .unwrap();
        assert!(!report.complete);
        assert_eq!(report.incomplete_reason, Some(IncompleteReason::Cancelled));
    }

    #[test]
    fn header_override_absent_keeps_first_row_as_data() {
        let mut request = ScanRequest::full();
        request.header = HeaderChoice::Absent;
        let report = scan_text("a,b\n1,2\n", &request);
        assert!(!report.header.present);
        assert_eq!(report.total_records, Some(2));
        assert_eq!(report.preview_rows[0].cells[0], "a");
    }

    #[test]
    fn preview_char_budget_is_bounded() {
        let mut text = String::from("value\n");
        for _ in 0..100 {
            text.push_str(&"x".repeat(500));
            text.push('\n');
        }
        let mut request = ScanRequest::preview();
        request.preview_rows = 100;
        request.limits.max_records = Some(100);
        request.limits.max_bytes = None;
        request.limits.max_preview_chars = 1024;
        let report = scan_text(&text, &request);
        let total: usize = report
            .preview_rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| cell.chars().count())
            .sum();
        assert!(total <= 1024 + 500, "preview budget was {total}");
        assert!(!report.preview_rows.is_empty());
    }

    #[test]
    fn header_text_is_bounded_in_the_report() {
        let long = "h".repeat(1000);
        let text = format!("{long},b\n1,2\n");
        let report = scan_text(&text, &ScanRequest::full());
        assert!(report.columns[0].name.chars().count() <= 200);
        assert!(report.header.raw[0].chars().count() <= 200);
    }

    #[test]
    fn column_names_are_unique() {
        let mut request = ScanRequest::full();
        request.header = HeaderChoice::Present;
        let report = scan_text("a,a,\n1,2,3\n", &request);
        assert_eq!(report.columns[1].name, "a (2)");
        assert_eq!(report.columns[2].name, "column_3");
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::HeaderDuplicate));
    }
}
