// SPDX-License-Identifier: Apache-2.0

//! Bounded preview and explicit full-scan session.
//!
//! A preview handler receives a stream, not a path. This module reads a
//! bounded head synchronously for the first screen, then copies the rest into a
//! private temporary spool with a hard byte cap. A full scan is a separate,
//! explicitly requested operation executed by a [`FullScanBackend`] (normally
//! the separate worker) and can be cancelled or timed out.
//!
//! Only the head is read on the calling thread; the rest is read on the
//! session's own thread. [`PreviewSession::start_with_head`] lets the Windows
//! handler read the head from the host apartment and then hand over a stream
//! that is only safe to read on another thread (a marshalled COM stream).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use csvview_core::{
    scan_file_with_info, scan_with_progress, InputInfo, ScanProgress, ScanReport, ScanRequest,
};
use tempfile::TempDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Spooling,
    SpoolReady,
    Scanning,
    Complete,
    Failed,
    TimedOut,
    Cancelled,
}

impl SessionPhase {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            SessionPhase::Complete
                | SessionPhase::Failed
                | SessionPhase::TimedOut
                | SessionPhase::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpoolProgress {
    pub bytes: u64,
    pub total_bytes: Option<u64>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpoolSource {
    pub path: PathBuf,
    pub bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub phase: SessionPhase,
    pub preview: Option<Arc<ScanReport>>,
    pub spool: SpoolProgress,
    pub spool_source: Option<SpoolSource>,
    pub progress: Option<ScanProgress>,
    pub full_report: Option<Arc<ScanReport>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    Io(String),
    Scan(String),
    Config(String),
    Busy,
    NoSpool,
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreviewError::Io(message) => write!(formatter, "I/O error: {message}"),
            PreviewError::Scan(message) => write!(formatter, "scan error: {message}"),
            PreviewError::Config(message) => write!(formatter, "configuration error: {message}"),
            PreviewError::Busy => write!(formatter, "a scan is already running"),
            PreviewError::NoSpool => write!(formatter, "the source spool is not ready"),
        }
    }
}

impl std::error::Error for PreviewError {}

#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// Bytes kept for the immediate first screen.
    pub head_bytes: usize,
    /// Hard cap for the private spool. Reaching it makes later scans partial.
    pub max_spool_bytes: u64,
    pub preview_rows: usize,
    pub spool_chunk_bytes: usize,
    pub scan_timeout: Duration,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            head_bytes: 256 * 1024,
            max_spool_bytes: 4 * 1024 * 1024 * 1024,
            preview_rows: 200,
            spool_chunk_bytes: 64 * 1024,
            scan_timeout: Duration::from_secs(120),
        }
    }
}

impl PreviewConfig {
    fn validate(&self) -> Result<(), PreviewError> {
        if self.head_bytes == 0 || self.preview_rows == 0 {
            return Err(PreviewError::Config(
                "head_bytes and preview_rows must be greater than zero".to_string(),
            ));
        }
        if self.max_spool_bytes == 0 || self.spool_chunk_bytes == 0 {
            return Err(PreviewError::Config(
                "spool limits must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

/// Copy a non-Send source on its owning thread into a bounded temporary file.
/// The extra byte lets the normal spool detect truncation at the limit.
pub fn copy_owned_source<R: Read>(source: R, limit: u64) -> io::Result<File> {
    use std::io::{Seek, SeekFrom};
    let mut file = tempfile::tempfile()?;
    io::copy(&mut source.take(limit.saturating_add(1)), &mut file)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

/// A full-scan implementation. Production uses the separate worker; tests and
/// the companion viewer can use the in-process implementation.
pub trait FullScanBackend: Send + Sync {
    fn scan(
        &self,
        spool: &SpoolSource,
        request: &ScanRequest,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, String>;
}

pub struct InProcessBackend;

impl FullScanBackend for InProcessBackend {
    fn scan(
        &self,
        spool: &SpoolSource,
        request: &ScanRequest,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, String> {
        let info = InputInfo {
            total_bytes: Some(spool.bytes),
            truncated: spool.truncated,
            preview_only: false,
        };
        scan_file_with_info(&spool.path, info, request, cancel, progress)
            .map_err(|error| error.to_string())
    }
}

struct Inner {
    state: Mutex<State>,
    changed: Condvar,
    cancel: AtomicBool,
    _directory: Arc<TempDir>,
}

struct State {
    phase: SessionPhase,
    preview: Option<Arc<ScanReport>>,
    spool: SpoolProgress,
    spool_source: Option<SpoolSource>,
    progress: Option<ScanProgress>,
    full_report: Option<Arc<ScanReport>>,
    error: Option<String>,
}

pub struct PreviewSession {
    inner: Arc<Inner>,
}

impl PreviewSession {
    /// Read a bounded head from `source` on the calling thread, publish the
    /// first preview immediately, and spool the rest on a private thread. The
    /// calling thread never copies the whole stream.
    pub fn start<R: Read + Send + 'static>(
        mut source: R,
        total_bytes: Option<u64>,
        request: &ScanRequest,
        config: PreviewConfig,
    ) -> Result<Self, PreviewError> {
        config.validate()?;
        let head = read_head(&mut source, config.head_bytes)?;
        Self::start_with_head(head, source, total_bytes, request, config)
    }

    /// As [`PreviewSession::start`], but with the head already read on the
    /// calling thread. The Windows handler reads the head from the host stream
    /// before marshalling the rest to this session's spool thread, so the host
    /// apartment is never blocked for a whole-file copy. `source` must be
    /// positioned immediately after `head`.
    pub fn start_with_head<R: Read + Send + 'static>(
        head: Vec<u8>,
        source: R,
        total_bytes: Option<u64>,
        request: &ScanRequest,
        config: PreviewConfig,
    ) -> Result<Self, PreviewError> {
        config.validate()?;

        let mut preview_request = request.clone();
        preview_request.preview_rows = config.preview_rows;
        preview_request.limits.max_records = Some(config.preview_rows as u64);
        // The head itself is the byte bound for the immediate preview.
        preview_request.limits.max_bytes = None;
        preview_request.limits.deadline_ms = Some(5_000);
        let preview = scan_with_progress(
            io::Cursor::new(head.clone()),
            InputInfo::preview(total_bytes.or(Some(head.len() as u64))),
            &preview_request,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .map_err(|error| PreviewError::Scan(error.to_string()))?;

        let directory = Arc::new(
            tempfile::Builder::new()
                .prefix("csv-viewer-")
                .tempdir()
                .map_err(|error| PreviewError::Io(error.to_string()))?,
        );
        let path = directory.path().join("source.csv");
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                phase: SessionPhase::Spooling,
                preview: Some(Arc::new(preview)),
                spool: SpoolProgress {
                    bytes: 0,
                    total_bytes,
                    truncated: false,
                },
                spool_source: None,
                progress: None,
                full_report: None,
                error: None,
            }),
            changed: Condvar::new(),
            cancel: AtomicBool::new(false),
            _directory: directory,
        });

        let thread_inner = inner.clone();
        let chunk = config.spool_chunk_bytes;
        let max_spool = config.max_spool_bytes;
        crate::lifecycle::spawn(
            thread::Builder::new().name("csv-preview-spool".to_string()),
            move || spool_stream(thread_inner, source, head, path, max_spool, chunk),
        )
        .map_err(|error| PreviewError::Io(error.to_string()))?;
        Ok(Self { inner })
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let state = self.inner.state.lock().unwrap();
        SessionSnapshot {
            phase: state.phase,
            preview: state.preview.clone(),
            spool: state.spool,
            spool_source: state.spool_source.clone(),
            progress: state.progress,
            full_report: state.full_report.clone(),
            error: state.error.clone(),
        }
    }

    /// Wait until copying finishes (or fails/cancels). The immediate preview
    /// does not require this.
    pub fn wait_for_spool(&self, timeout: Duration) -> bool {
        let state = self.inner.state.lock().unwrap();
        let (state, _) = self
            .inner
            .changed
            .wait_timeout_while(state, timeout, |state| {
                state.phase == SessionPhase::Spooling
            })
            .unwrap();
        state.phase != SessionPhase::Spooling
    }

    pub fn wait_for_scan(&self, timeout: Duration) -> bool {
        let state = self.inner.state.lock().unwrap();
        let (state, _) = self
            .inner
            .changed
            .wait_timeout_while(state, timeout, |state| {
                state.phase == SessionPhase::Scanning
            })
            .unwrap();
        state.phase != SessionPhase::Scanning
    }

    fn scheduling_failed(&self, error: std::io::Error) -> PreviewError {
        let message = error.to_string();
        let mut state = self.inner.state.lock().unwrap();
        if state.phase == SessionPhase::Scanning {
            self.inner.cancel.store(true, Ordering::SeqCst);
            state.phase = SessionPhase::Failed;
            state.error = Some(message.clone());
            self.inner.changed.notify_all();
        }
        PreviewError::Io(message)
    }
    pub fn start_full_scan(
        &self,
        backend: Arc<dyn FullScanBackend>,
        request: ScanRequest,
        timeout: Duration,
    ) -> Result<(), PreviewError> {
        let spool = {
            let mut state = self.inner.state.lock().unwrap();
            if matches!(state.phase, SessionPhase::Scanning | SessionPhase::Complete) {
                return Err(PreviewError::Busy);
            }
            if state.phase != SessionPhase::SpoolReady {
                return Err(PreviewError::NoSpool);
            }
            let spool = state.spool_source.clone().ok_or(PreviewError::NoSpool)?;
            state.phase = SessionPhase::Scanning;
            state.progress = None;
            state.error = None;
            self.inner.cancel.store(false, Ordering::SeqCst);
            self.inner.changed.notify_all();
            spool
        };

        let thread_inner = self.inner.clone();
        crate::lifecycle::spawn(
            thread::Builder::new().name("csv-preview-scan".to_string()),
            move || {
                let result =
                    backend.scan(&spool, &request, &thread_inner.cancel, &mut |progress| {
                        let mut state = thread_inner.state.lock().unwrap();
                        if state.phase == SessionPhase::Scanning {
                            state.progress = Some(progress);
                            thread_inner.changed.notify_all();
                        }
                    });
                let mut state = thread_inner.state.lock().unwrap();
                if thread_inner.cancel.load(Ordering::SeqCst) {
                    if !state.phase.is_terminal() {
                        state.phase = SessionPhase::Cancelled;
                    }
                } else if state.phase != SessionPhase::TimedOut {
                    match result {
                        Ok(report) => {
                            state.full_report = Some(Arc::new(report));
                            state.phase = SessionPhase::Complete;
                        }
                        Err(error) => {
                            state.error = Some(error);
                            state.phase = SessionPhase::Failed;
                        }
                    }
                }
                thread_inner.changed.notify_all();
            },
        )
        .map_err(|error| self.scheduling_failed(error))?;

        let supervisor_inner = self.inner.clone();
        crate::lifecycle::spawn(
            thread::Builder::new().name("csv-preview-timeout".to_string()),
            move || {
                let deadline = Instant::now() + timeout;
                let mut state = supervisor_inner.state.lock().unwrap();
                loop {
                    if state.phase != SessionPhase::Scanning {
                        break;
                    }
                    let now = Instant::now();
                    if now >= deadline {
                        state.phase = SessionPhase::TimedOut;
                        supervisor_inner.cancel.store(true, Ordering::SeqCst);
                        state.error = Some("full scan timed out".to_string());
                        supervisor_inner.changed.notify_all();
                        break;
                    }
                    let (next, _) = supervisor_inner
                        .changed
                        .wait_timeout(state, deadline - now)
                        .unwrap();
                    state = next;
                }
            },
        )
        .map_err(|error| self.scheduling_failed(error))?;
        Ok(())
    }

    /// Cancel copying or a running full scan. The spool is deleted when the
    /// session is dropped.
    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::SeqCst);
        let mut state = self.inner.state.lock().unwrap();
        if matches!(
            state.phase,
            SessionPhase::Spooling | SessionPhase::SpoolReady | SessionPhase::Scanning
        ) {
            state.phase = SessionPhase::Cancelled;
        }
        self.inner.changed.notify_all();
    }
}

impl Drop for PreviewSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Read at most `max` bytes from `source`. The Windows handler uses this to
/// read the head on the host apartment before marshalling the rest.
pub fn read_head<R: Read>(source: &mut R, max: usize) -> Result<Vec<u8>, PreviewError> {
    let mut head = Vec::with_capacity(max);
    let mut buffer = [0u8; 8192];
    while head.len() < max {
        let remaining = max - head.len();
        let take = remaining.min(buffer.len());
        let read = source
            .read(&mut buffer[..take])
            .map_err(|error| PreviewError::Io(error.to_string()))?;
        if read == 0 {
            break;
        }
        head.extend_from_slice(&buffer[..read]);
    }
    Ok(head)
}

fn spool_stream<R: Read>(
    inner: Arc<Inner>,
    mut source: R,
    head: Vec<u8>,
    path: PathBuf,
    max_spool: u64,
    chunk: usize,
) {
    let result = write_spool(&inner, &mut source, &head, &path, max_spool, chunk);
    let mut state = inner.state.lock().unwrap();
    match result {
        Ok((bytes, truncated)) => {
            if state.phase == SessionPhase::Spooling && !inner.cancel.load(Ordering::SeqCst) {
                let total = if truncated {
                    Some(bytes).filter(|bytes| *bytes >= max_spool)
                } else {
                    Some(bytes)
                };
                state.spool = SpoolProgress {
                    bytes,
                    total_bytes: total,
                    truncated,
                };
                state.spool_source = Some(SpoolSource {
                    path,
                    bytes,
                    truncated,
                });
                state.phase = SessionPhase::SpoolReady;
            }
        }
        Err(error) => {
            if state.phase == SessionPhase::Spooling {
                state.error = Some(error.to_string());
                state.phase = SessionPhase::Failed;
            }
        }
    }
    inner.changed.notify_all();
}

fn write_spool<R: Read>(
    inner: &Inner,
    source: &mut R,
    head: &[u8],
    path: &PathBuf,
    max_spool: u64,
    chunk: usize,
) -> io::Result<(u64, bool)> {
    let mut file = File::create(path)?;
    let mut bytes = 0u64;
    let mut truncated = false;

    let head_take = head.len().min(max_spool as usize);
    file.write_all(&head[..head_take])?;
    bytes += head_take as u64;
    if head.len() as u64 > max_spool {
        truncated = true;
    } else {
        let mut buffer = vec![0u8; chunk];
        loop {
            if inner.cancel.load(Ordering::SeqCst) {
                return Ok((bytes, truncated));
            }
            let allowed = (max_spool - bytes) as usize;
            if allowed == 0 {
                // 07A-09: the cap is filled exactly. A source that yields
                // nothing more is fully spooled, not truncated; a single
                // further byte proves the source outgrew the cap. The +1
                // byte copy (`copy_owned_source`) feeds this probe.
                let mut probe_slot = [0u8; 1];
                let probe = source.read(&mut probe_slot)?;
                truncated = probe > 0;
                break;
            }
            let take = allowed.min(buffer.len());
            let read = source.read(&mut buffer[..take])?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            bytes += read as u64;
            let mut state = inner.state.lock().unwrap();
            let total_bytes = state.spool.total_bytes;
            state.spool = SpoolProgress {
                bytes,
                total_bytes,
                truncated: false,
            };
            inner.changed.notify_all();
        }
    }
    file.flush()?;
    file.sync_data()?;
    Ok((bytes, truncated))
}

#[cfg(test)]
mod owned_source_tests {
    use super::*;
    fn ready_session() -> PreviewSession {
        let session = PreviewSession::start(
            std::io::Cursor::new(b"name\nvalue\n".to_vec()),
            Some(11),
            &ScanRequest::preview(),
            PreviewConfig::default(),
        )
        .unwrap();
        assert!(session.wait_for_spool(Duration::from_secs(5)));
        session
    }
    #[test]
    fn scheduling_failure_fails_and_wakes_scanning_without_overwriting_terminal_state() {
        let session = ready_session();
        session.inner.state.lock().unwrap().phase = SessionPhase::Scanning;
        session.scheduling_failed(std::io::Error::other("thread creation failed"));
        assert_eq!(session.snapshot().phase, SessionPhase::Failed);
        assert!(session.wait_for_scan(Duration::ZERO));
        assert!(session.inner.cancel.load(Ordering::SeqCst));
        session.inner.state.lock().unwrap().phase = SessionPhase::Complete;
        session.scheduling_failed(std::io::Error::other("late failure"));
        assert_eq!(session.snapshot().phase, SessionPhase::Complete);
    }
    #[test]
    fn dropping_a_session_wakes_supervision_into_a_terminal_state() {
        let session = ready_session();
        let inner = session.inner.clone();
        inner.state.lock().unwrap().phase = SessionPhase::Scanning;
        drop(session);
        assert_eq!(inner.state.lock().unwrap().phase, SessionPhase::Cancelled);
        assert!(inner.cancel.load(Ordering::SeqCst));
    }
    #[test]
    fn copy_is_bounded_and_rewound() {
        let mut file = copy_owned_source(&b"abcdef"[..], 3).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        assert_eq!(data, b"abcd");
    }
}
