// SPDX-License-Identifier: Apache-2.0

//! Constrained CSV scan worker.
//!
//! The worker is a separate process. It only opens a regular file inside the
//! private spool directory it was started with, enforces its own caps on top of
//! the request's limits, and reports progress with a versioned protocol. It
//! never writes to the source, never touches the network, and never places
//! orders or calls trading APIs.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use csv_protocol::{
    read_message, write_message, ClientMessage, ErrorCode, FrameError, SourceSpec, WorkerMessage,
    PROTOCOL_VERSION,
};
use csvview_core::{scan_file_with_info, InputInfo, ScanError, ScanRequest};

const MAX_PENDING_REQUESTS: usize = 8;
const DEFAULT_MAX_FRAME: usize = csv_protocol::frame::MAX_FRAME_BYTES;
const DEFAULT_MAX_SPOOL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_MAX_RECORDS: u64 = 100_000_000;
const DEFAULT_MAX_SCAN_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_MAX_DEADLINE_MS: u64 = 120_000;

#[derive(Debug, Clone)]
struct Config {
    spool_dir: PathBuf,
    max_frame_bytes: usize,
    max_spool_bytes: u64,
    max_records: u64,
    max_scan_bytes: u64,
    max_deadline_ms: u64,
}

impl Config {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut spool_dir = None;
        let mut config = Self {
            spool_dir: PathBuf::new(),
            max_frame_bytes: DEFAULT_MAX_FRAME,
            max_spool_bytes: DEFAULT_MAX_SPOOL_BYTES,
            max_records: DEFAULT_MAX_RECORDS,
            max_scan_bytes: DEFAULT_MAX_SCAN_BYTES,
            max_deadline_ms: DEFAULT_MAX_DEADLINE_MS,
        };
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--spool-dir" => {
                    spool_dir = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--spool-dir needs a value".to_string())?,
                    );
                }
                "--max-frame-bytes" => {
                    config.max_frame_bytes =
                        parse_number(&mut arguments, "--max-frame-bytes")? as usize;
                }
                "--max-spool-bytes" => {
                    config.max_spool_bytes = parse_number(&mut arguments, "--max-spool-bytes")?;
                }
                "--max-records" => {
                    config.max_records = parse_number(&mut arguments, "--max-records")?;
                }
                "--max-bytes" => {
                    config.max_scan_bytes = parse_number(&mut arguments, "--max-bytes")?;
                }
                "--max-deadline-ms" => {
                    config.max_deadline_ms = parse_number(&mut arguments, "--max-deadline-ms")?;
                }
                "--help" | "-h" => {
                    println!(
                        "csv-worker {}\n\nUsage: csv-worker --spool-dir <dir> [options]\n\nOptions:\n  --max-frame-bytes N   maximum protocol frame (default 16 MiB)\n  --max-spool-bytes N   maximum source spool size (default 4 GiB)\n  --max-records N       worker record cap (default 100,000,000)\n  --max-bytes N         worker source-byte cap (default 4 GiB)\n  --max-deadline-ms N   worker time cap (default 120,000)\n\nMade by Maria. Apache-2.0.",
                        env!("CARGO_PKG_VERSION")
                    );
                    std::process::exit(0);
                }
                "--version" => {
                    println!("csv-worker {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        let spool_dir = spool_dir.ok_or_else(|| "--spool-dir is required".to_string())?;
        config.spool_dir = PathBuf::from(spool_dir);
        if config.max_frame_bytes < 1024 {
            return Err("--max-frame-bytes must be at least 1024".to_string());
        }
        if config.max_spool_bytes == 0 || config.max_records == 0 || config.max_scan_bytes == 0 {
            return Err("worker limits must be greater than zero".to_string());
        }
        Ok(config)
    }
}

fn parse_number(
    arguments: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<u64, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{name} needs a value"))?;
    value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a non-negative integer"))
}

#[derive(Default)]
struct ControlState {
    current: Option<u64>,
    // One entry per request actually received, not arbitrary Cancel messages.
    pending: BTreeMap<u64, VecDeque<bool>>,
    fatal: Option<String>,
}

#[derive(Default)]
struct Control {
    state: Mutex<ControlState>,
    flag: AtomicBool,
}

impl Control {
    fn abort(&self, message: &str) {
        self.state.lock().unwrap().fatal = Some(message.to_string());
        self.flag.store(true, Ordering::SeqCst);
    }

    fn failure(&self) -> Option<String> {
        self.state.lock().unwrap().fatal.clone()
    }

    fn queue(&self, id: u64) {
        self.state
            .lock()
            .unwrap()
            .pending
            .entry(id)
            .or_default()
            .push_back(false);
    }

    fn begin(&self, id: u64) {
        let mut state = self.state.lock().unwrap();
        let queue = state
            .pending
            .get_mut(&id)
            .expect("scan registered before dispatch");
        let cancelled = queue.pop_front().expect("queued scan");
        if queue.is_empty() {
            state.pending.remove(&id);
        }
        state.current = Some(id);
        self.flag
            .store(cancelled || state.fatal.is_some(), Ordering::SeqCst);
    }

    fn cancel(&self, id: u64) {
        let mut state = self.state.lock().unwrap();
        if state.current == Some(id) {
            self.flag.store(true, Ordering::SeqCst);
        } else if let Some(cancelled) = state.pending.get_mut(&id).and_then(|q| q.front_mut()) {
            *cancelled = true;
        }
        // Unknown/completed IDs are ignored, so a late cancellation cannot
        // persist until an unrelated future reuse of that ID.
    }

    fn end(&self) {
        let mut state = self.state.lock().unwrap();
        state.current = None;
        self.flag.store(false, Ordering::SeqCst);
    }
}

// Never block the input reader on a full work queue: it must remain able to
// cancel the active scan. Overflow cancels it and terminates the connection.
fn queue_message(
    sender: &std::sync::mpsc::SyncSender<Result<ClientMessage, FrameError>>,
    control: &Control,
    message: Result<ClientMessage, FrameError>,
) -> bool {
    if let Ok(ClientMessage::Scan { id, .. }) = &message {
        control.queue(*id);
    }
    match sender.try_send(message) {
        Ok(()) => true,
        Err(std::sync::mpsc::TrySendError::Full(_)) => {
            control.abort("too many pending worker requests (maximum 8); connection closed");
            false
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => false,
    }
}

fn main() {
    let config = match Config::parse(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("csv-worker: {message}");
            std::process::exit(2);
        }
    };
    if let Err(message) = run(config) {
        eprintln!("csv-worker: {message}");
        std::process::exit(1);
    }
}

fn run(config: Config) -> Result<(), String> {
    let spool_dir = config
        .spool_dir
        .canonicalize()
        .map_err(|error| format!("cannot resolve spool directory: {error}"))?;
    if !spool_dir.is_dir() {
        return Err("spool directory is not a directory".to_string());
    }

    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut input = BufReader::new(io::stdin());
    let hello: ClientMessage = match read_message(&mut input, config.max_frame_bytes) {
        Ok(message) => message,
        Err(error) => return Err(format!("handshake failed: {error}")),
    };
    match hello {
        ClientMessage::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => {}
        ClientMessage::Hello { protocol_version } => {
            send_error(
                &mut output,
                None,
                ErrorCode::BadVersion,
                format!(
                    "protocol version {protocol_version} is not supported (expected {PROTOCOL_VERSION})"
                ),
                config.max_frame_bytes,
            )?;
            return Ok(());
        }
        _ => {
            send_error(
                &mut output,
                None,
                ErrorCode::BadRequest,
                "expected a hello message".to_string(),
                config.max_frame_bytes,
            )?;
            return Ok(());
        }
    }
    write_message(
        &mut output,
        &WorkerMessage::Ready {
            protocol_version: PROTOCOL_VERSION,
            max_frame_bytes: config.max_frame_bytes as u64,
            pid: std::process::id(),
        },
        config.max_frame_bytes,
    )
    .map_err(|error| format!("cannot send ready: {error}"))?;

    let control = Arc::new(Control::default());
    let (sender, receiver) =
        std::sync::mpsc::sync_channel::<Result<ClientMessage, FrameError>>(MAX_PENDING_REQUESTS);
    let reader_config = config.clone();
    let reader_control = control.clone();
    let _reader = thread::spawn(move || loop {
        match read_message::<_, ClientMessage>(&mut input, reader_config.max_frame_bytes) {
            Ok(ClientMessage::Cancel { id }) => reader_control.cancel(id),
            Ok(message) => {
                if !queue_message(&sender, &reader_control, Ok(message)) {
                    break;
                }
            }
            Err(FrameError::Closed) => break,
            Err(error) => {
                queue_message(&sender, &reader_control, Err(error));
                break;
            }
        }
    });

    for message in receiver {
        if let Some(error) = control.failure() {
            send_error(
                &mut output,
                None,
                ErrorCode::BadRequest,
                error.clone(),
                config.max_frame_bytes,
            )?;
            return Err(error);
        }
        match message {
            Ok(ClientMessage::Scan {
                id,
                source,
                request,
            }) => {
                control.begin(id);
                let result = perform_scan(
                    id,
                    &source,
                    &request,
                    &spool_dir,
                    &config,
                    &control.flag,
                    &mut output,
                );
                finish_scan(&control, id, result, &mut output, config.max_frame_bytes)?;
            }
            Ok(ClientMessage::Shutdown) => {
                write_message(&mut output, &WorkerMessage::Goodbye, config.max_frame_bytes)
                    .map_err(|error| format!("cannot send goodbye: {error}"))?;
                break;
            }
            Ok(ClientMessage::Hello { .. }) => {
                send_error(
                    &mut output,
                    None,
                    ErrorCode::BadRequest,
                    "already greeted".to_string(),
                    config.max_frame_bytes,
                )?;
            }
            Ok(ClientMessage::Cancel { id }) => control.cancel(id),
            Err(error) => {
                send_error(
                    &mut output,
                    None,
                    ErrorCode::Io,
                    error.to_string(),
                    config.max_frame_bytes,
                )?;
                break;
            }
        }
    }
    Ok(())
}

enum WorkerError {
    Output(FrameError),
    Message(ErrorCode, String),
}

fn finish_scan(
    control: &Control,
    id: u64,
    result: Result<WorkerMessage, WorkerError>,
    output: &mut impl Write,
    max_frame_bytes: usize,
) -> Result<(), String> {
    // Publish completion internally BEFORE the peer can observe its terminal
    // reply and reuse the ID. A cancel for a newly queued reuse then cannot
    // be consumed by the completed scan.
    control.end();
    let response = match result {
        Ok(response) => response,
        Err(WorkerError::Message(code, message)) => WorkerMessage::Error {
            id: Some(id),
            code,
            message,
        },
        Err(WorkerError::Output(error)) => return Err(format!("scan output failed: {error}")),
    };
    write_message(output, &response, max_frame_bytes)
        .map(|_| ())
        .map_err(|error| format!("cannot send scan result: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn perform_scan(
    id: u64,
    source: &SourceSpec,
    request: &ScanRequest,
    spool_dir: &Path,
    config: &Config,
    cancel: &AtomicBool,
    output: &mut impl Write,
) -> Result<WorkerMessage, WorkerError> {
    let path = validate_source(source, spool_dir, config)?;
    write_message(
        output,
        &WorkerMessage::Accepted { id },
        config.max_frame_bytes,
    )
    .map_err(WorkerError::Output)?;

    let mut request = request.clone();
    cap_request(&mut request, config);
    request
        .validate()
        .map_err(|error| WorkerError::Message(ErrorCode::BadRequest, error.to_string()))?;

    let mut progress_error = None;
    let info = InputInfo {
        total_bytes: Some(source.bytes),
        truncated: source.truncated,
        preview_only: false,
    };
    let result = scan_file_with_info(&path, info, &request, cancel, &mut |progress| {
        if progress_error.is_none() {
            if let Err(error) = write_message(
                output,
                &WorkerMessage::Progress { id, progress },
                config.max_frame_bytes,
            ) {
                progress_error = Some(error);
                cancel.store(true, Ordering::SeqCst);
            }
        }
    });
    if let Some(error) = progress_error {
        return Err(WorkerError::Output(error));
    }
    match result {
        Ok(report) => Ok(WorkerMessage::Result { id, report }),
        Err(ScanError::InvalidRequest(message)) => {
            Err(WorkerError::Message(ErrorCode::BadRequest, message))
        }
        Err(ScanError::Io(message)) => Err(WorkerError::Message(ErrorCode::Io, message)),
    }
}

fn cap_request(request: &mut ScanRequest, config: &Config) {
    let limits = &mut request.limits;
    limits.max_records = Some(
        limits
            .max_records
            .unwrap_or(config.max_records)
            .min(config.max_records),
    );
    limits.max_bytes = Some(
        limits
            .max_bytes
            .unwrap_or(config.max_scan_bytes)
            .min(config.max_scan_bytes),
    );
    limits.deadline_ms = Some(
        limits
            .deadline_ms
            .unwrap_or(config.max_deadline_ms)
            .min(config.max_deadline_ms),
    );
    limits.max_field_chars = limits.max_field_chars.min(1_000_000);
    limits.max_columns = limits.max_columns.min(4096);
    limits.max_preview_chars = limits.max_preview_chars.clamp(1024, 2 * 1024 * 1024);
    request.preview_rows = request.preview_rows.min(1000);
}

fn validate_source(
    source: &SourceSpec,
    spool_dir: &Path,
    config: &Config,
) -> Result<PathBuf, WorkerError> {
    if source.bytes > config.max_spool_bytes {
        return Err(WorkerError::Message(
            ErrorCode::SourceTooLarge,
            format!(
                "source size {} exceeds worker limit {}",
                source.bytes, config.max_spool_bytes
            ),
        ));
    }
    let path = PathBuf::from(&source.path);
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        WorkerError::Message(
            ErrorCode::Io,
            format!("cannot read source metadata: {error}"),
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(WorkerError::Message(
            ErrorCode::BadRequest,
            "source is not a regular file".to_string(),
        ));
    }
    if metadata.len() != source.bytes {
        return Err(WorkerError::Message(
            ErrorCode::SourceSizeMismatch,
            format!(
                "source length {} does not match declared length {}",
                metadata.len(),
                source.bytes
            ),
        ));
    }
    let canonical = path.canonicalize().map_err(|error| {
        WorkerError::Message(
            ErrorCode::Io,
            format!("cannot resolve source path: {error}"),
        )
    })?;
    if !canonical.starts_with(spool_dir) {
        return Err(WorkerError::Message(
            ErrorCode::SourceOutsideSpool,
            "source is outside the worker spool directory".to_string(),
        ));
    }
    Ok(canonical)
}

fn send_error(
    output: &mut impl Write,
    id: Option<u64>,
    code: ErrorCode,
    message: String,
    max_frame_bytes: usize,
) -> Result<(), String> {
    write_message(
        output,
        &WorkerMessage::Error { id, code, message },
        max_frame_bytes,
    )
    .map(|_| ())
    .map_err(|error| format!("cannot send error response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            spool_dir: PathBuf::from("/tmp"),
            max_frame_bytes: DEFAULT_MAX_FRAME,
            max_spool_bytes: 1024,
            max_records: 10,
            max_scan_bytes: 1024,
            max_deadline_ms: 1000,
        }
    }

    #[test]
    fn caps_are_applied() {
        let mut request = ScanRequest::full();
        request.limits.max_records = Some(1_000);
        request.preview_rows = 10_000;
        cap_request(&mut request, &config());
        assert_eq!(request.limits.max_records, Some(10));
        assert_eq!(request.limits.max_bytes, Some(1024));
        assert_eq!(request.limits.deadline_ms, Some(1000));
        assert_eq!(request.preview_rows, 1000);
    }

    #[test]
    fn missing_caps_use_worker_defaults() {
        let mut request = ScanRequest::default();
        request.limits.max_records = None;
        request.limits.max_bytes = None;
        request.limits.deadline_ms = None;
        cap_request(&mut request, &config());
        assert_eq!(request.limits.max_records, Some(10));
        assert_eq!(request.limits.max_bytes, Some(1024));
        assert_eq!(request.limits.deadline_ms, Some(1000));
    }

    #[test]
    fn rejects_paths_outside_the_spool() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        let source = SourceSpec {
            path: outside.path().to_string_lossy().into_owned(),
            bytes: std::fs::metadata(outside.path()).unwrap().len(),
            truncated: false,
        };
        let config = Config {
            spool_dir: directory.path().to_path_buf(),
            ..config()
        };
        let error = validate_source(&source, directory.path(), &config).unwrap_err();
        assert!(matches!(
            error,
            WorkerError::Message(ErrorCode::SourceOutsideSpool, _)
        ));
    }

    #[test]
    fn rejects_size_mismatch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("spool.csv");
        std::fs::write(&path, b"a,b\n").unwrap();
        let source = SourceSpec {
            path: path.to_string_lossy().into_owned(),
            bytes: 99,
            truncated: false,
        };
        let config = Config {
            spool_dir: directory.path().to_path_buf(),
            ..config()
        };
        let error = validate_source(&source, directory.path(), &config).unwrap_err();
        assert!(matches!(
            error,
            WorkerError::Message(ErrorCode::SourceSizeMismatch, _)
        ));
    }

    #[test]
    fn accepts_a_regular_file_inside_the_spool() {
        let directory = tempfile::tempdir().unwrap();
        // Production canonicalizes the spool directory at startup; do the same here so the
        // comparison also holds on Windows, where canonical paths use the `\\?\` form.
        let spool_dir = directory.path().canonicalize().unwrap();
        let path = spool_dir.join("spool.csv");
        std::fs::write(&path, b"a,b\n").unwrap();
        let source = SourceSpec {
            path: path.to_string_lossy().into_owned(),
            bytes: 4,
            truncated: false,
        };
        let config = Config {
            spool_dir: spool_dir.clone(),
            ..config()
        };
        assert!(validate_source(&source, &spool_dir, &config).is_ok());
    }

    #[test]
    fn control_cancels_only_the_matching_scan() {
        let control = Control::default();
        control.queue(1);
        control.begin(1);
        control.queue(2);
        control.cancel(2);
        assert!(!control.flag.load(Ordering::SeqCst));
        control.cancel(1);
        assert!(control.flag.load(Ordering::SeqCst));
        control.end();
        control.queue(1);
        control.begin(1);
        assert!(!control.flag.load(Ordering::SeqCst));
        control.end();
        control.begin(2);
        assert!(
            control.flag.load(Ordering::SeqCst),
            "pending cancel is kept"
        );
        control.end();
        control.queue(2);
        control.begin(2);
        assert!(!control.flag.load(Ordering::SeqCst));
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn request_flood_is_bounded_and_cancels_active_work() {
        let control = Control::default();
        let (sender, receiver) = std::sync::mpsc::sync_channel(MAX_PENDING_REQUESTS);
        control.queue(0);
        control.begin(0);
        for id in 1..=MAX_PENDING_REQUESTS as u64 {
            let message = ClientMessage::Scan {
                id,
                source: SourceSpec {
                    path: "synthetic".into(),
                    bytes: 0,
                    truncated: false,
                },
                request: ScanRequest::full(),
            };
            assert!(queue_message(&sender, &control, Ok(message)));
        }
        assert!(!queue_message(
            &sender,
            &control,
            Ok(ClientMessage::Shutdown)
        ));
        assert!(control.flag.load(Ordering::SeqCst));
        assert!(control.failure().unwrap().contains("too many pending"));
        assert_eq!(receiver.try_iter().count(), MAX_PENDING_REQUESTS);
        assert_eq!(
            control.state.lock().unwrap().pending.len(),
            MAX_PENDING_REQUESTS
        );
    }

    #[test]
    fn partial_terminal_and_error_writes_are_fatal() {
        struct Broken {
            left: usize,
        }
        impl Write for Broken {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.left == 0 {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"));
                }
                let n = bytes.len().min(self.left);
                self.left -= n;
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        for left in [0, 2, 6] {
            let control = Control::default();
            control.queue(7);
            control.begin(7);
            assert!(finish_scan(
                &control,
                7,
                Err(WorkerError::Message(
                    ErrorCode::BadRequest,
                    "bad scan".into()
                )),
                &mut Broken { left },
                DEFAULT_MAX_FRAME
            )
            .is_err());
            assert!(control.state.lock().unwrap().current.is_none());
            assert!(send_error(
                &mut Broken { left },
                None,
                ErrorCode::BadRequest,
                "bad input".into(),
                DEFAULT_MAX_FRAME
            )
            .is_err());
        }
    }
    #[test]
    fn result_delivery_reuse_and_cancel_targets_the_new_request() {
        // The writer acts as a synchronized peer: once the terminal Result
        // begins delivery, immediately queue and cancel a replacement.
        struct Peer<'a> {
            control: &'a Control,
            bytes: Vec<u8>,
            queued: bool,
        }
        impl Write for Peer<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if !self.queued {
                    self.control.queue(7);
                    self.control.cancel(7);
                    self.queued = true;
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("result.csv");
        std::fs::write(&path, b"a,b\n1,2\n").unwrap();
        let config = Config::parse(
            [
                "--spool-dir".to_string(),
                root.to_string_lossy().into_owned(),
            ]
            .into_iter(),
        )
        .unwrap();
        let source = SourceSpec {
            path: path.to_string_lossy().into_owned(),
            bytes: 8,
            truncated: false,
        };
        let control = Control::default();
        control.queue(7);
        control.begin(7);
        let result = perform_scan(
            7,
            &source,
            &ScanRequest::full(),
            &root,
            &config,
            &control.flag,
            &mut Vec::new(),
        );
        assert!(matches!(&result, Ok(WorkerMessage::Result { .. })));
        let mut peer = Peer {
            control: &control,
            bytes: Vec::new(),
            queued: false,
        };
        finish_scan(&control, 7, result, &mut peer, DEFAULT_MAX_FRAME).unwrap();
        let reply: WorkerMessage =
            read_message(&mut io::Cursor::new(peer.bytes), DEFAULT_MAX_FRAME).unwrap();
        assert!(matches!(reply, WorkerMessage::Result { id: 7, .. }));
        control.begin(7);
        assert!(
            control.flag.load(Ordering::SeqCst),
            "new queued request lost its cancellation"
        );
    }
    #[test]
    fn late_or_unknown_cancel_is_not_saved_for_future_reuse() {
        let control = Control::default();
        control.queue(7);
        control.begin(7);
        control.end();
        control.cancel(7);
        for id in 100..10_000 {
            control.cancel(id);
        }
        assert!(control.state.lock().unwrap().pending.is_empty());
        control.queue(7);
        control.begin(7);
        assert!(!control.flag.load(Ordering::SeqCst));
        control.end();
    }

    #[test]
    fn queued_cancel_is_consumed_only_by_the_received_request() {
        let control = Control::default();
        control.queue(1);
        control.begin(1);
        control.queue(2);
        control.cancel(2);
        assert!(!control.flag.load(Ordering::SeqCst));
        control.end();
        control.begin(2);
        assert!(control.flag.load(Ordering::SeqCst));
        control.end();
        control.cancel(2);
        control.queue(2);
        control.begin(2);
        assert!(!control.flag.load(Ordering::SeqCst));
    }

    #[test]
    fn next_request_may_arrive_while_previous_result_is_being_delivered() {
        let control = Control::default();
        control.queue(7);
        control.begin(7);
        control.cancel(7);
        control.queue(7);
        control.end();
        control.begin(7);
        assert!(!control.flag.load(Ordering::SeqCst));
    }
    #[test]
    fn active_cancel_does_not_cancel_a_later_reused_id() {
        let control = Control::default();
        control.queue(7);
        control.begin(7);
        control.cancel(7);
        assert!(control.flag.load(Ordering::SeqCst));
        control.end();
        control.queue(7);
        control.begin(7);
        assert!(
            !control.flag.load(Ordering::SeqCst),
            "active cancellation was retained for a new scan"
        );
        control.end();
    }

    #[test]
    fn cancelling_an_old_request_cannot_poison_the_next_request() {
        let control = Arc::new(Control::default());
        let gate = Arc::new(std::sync::Barrier::new(2));
        let other = control.clone();
        let bg = gate.clone();
        let t = thread::spawn(move || {
            for _ in 0..20000 {
                bg.wait();
                other.cancel(1);
                bg.wait();
            }
        });
        let mut bad = false;
        for _ in 0..20000 {
            control.queue(1);
            control.begin(1);
            gate.wait();
            control.end();
            control.queue(2);
            control.begin(2);
            gate.wait();
            bad |= control.flag.load(Ordering::Relaxed);
            control.end();
        }
        t.join().unwrap();
        assert!(!bad, "late cancel for request 1 must not cancel request 2");
    }
}
