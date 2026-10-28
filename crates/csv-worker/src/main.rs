// SPDX-License-Identifier: Apache-2.0

//! Constrained CSV scan worker.
//!
//! The worker is a separate process. It only opens a regular file inside the
//! private spool directory it was started with, enforces its own caps on top of
//! the request's limits, and reports progress with a versioned protocol. It
//! never writes to the source, never touches the network, and never places
//! orders or calls trading APIs.

use std::collections::BTreeSet;
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
struct Control {
    current: Mutex<Option<u64>>,
    cancelled: Mutex<BTreeSet<u64>>,
    flag: AtomicBool,
}

impl Control {
    fn begin(&self, id: u64) {
        self.flag.store(false, Ordering::SeqCst);
        let mut cancelled = self.cancelled.lock().unwrap();
        if cancelled.remove(&id) {
            self.flag.store(true, Ordering::SeqCst);
        }
        *self.current.lock().unwrap() = Some(id);
    }

    fn cancel(&self, id: u64) {
        self.cancelled.lock().unwrap().insert(id);
        if *self.current.lock().unwrap() == Some(id) {
            self.flag.store(true, Ordering::SeqCst);
        }
    }

    fn end(&self) {
        *self.current.lock().unwrap() = None;
        self.flag.store(false, Ordering::SeqCst);
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
            );
            return Ok(());
        }
        _ => {
            send_error(
                &mut output,
                None,
                ErrorCode::BadRequest,
                "expected a hello message".to_string(),
                config.max_frame_bytes,
            );
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
    let (sender, receiver) = std::sync::mpsc::channel::<Result<ClientMessage, FrameError>>();
    let reader_config = config.clone();
    let reader_control = control.clone();
    let _reader = thread::spawn(move || loop {
        match read_message::<_, ClientMessage>(&mut input, reader_config.max_frame_bytes) {
            Ok(ClientMessage::Cancel { id }) => reader_control.cancel(id),
            Ok(message) => {
                if sender.send(Ok(message)).is_err() {
                    break;
                }
            }
            Err(FrameError::Closed) => break,
            Err(error) => {
                let _ = sender.send(Err(error));
                break;
            }
        }
    });

    for message in receiver {
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
                control.end();
                match result {
                    Ok(()) => {}
                    Err(WorkerError::Reported) => {}
                    Err(WorkerError::Message(code, message)) => {
                        send_error(&mut output, Some(id), code, message, config.max_frame_bytes);
                    }
                }
            }
            Ok(ClientMessage::Shutdown) => {
                let _ = write_message(&mut output, &WorkerMessage::Goodbye, config.max_frame_bytes);
                break;
            }
            Ok(ClientMessage::Hello { .. }) => {
                send_error(
                    &mut output,
                    None,
                    ErrorCode::BadRequest,
                    "already greeted".to_string(),
                    config.max_frame_bytes,
                );
            }
            Ok(ClientMessage::Cancel { id }) => control.cancel(id),
            Err(error) => {
                send_error(
                    &mut output,
                    None,
                    ErrorCode::Io,
                    error.to_string(),
                    config.max_frame_bytes,
                );
                break;
            }
        }
    }
    Ok(())
}

enum WorkerError {
    Reported,
    Message(ErrorCode, String),
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
) -> Result<(), WorkerError> {
    let path = match validate_source(source, spool_dir, config) {
        Ok(path) => path,
        Err(WorkerError::Message(code, message)) => {
            send_error(output, Some(id), code, message, config.max_frame_bytes);
            return Err(WorkerError::Reported);
        }
        Err(error) => return Err(error),
    };

    if write_message(
        output,
        &WorkerMessage::Accepted { id },
        config.max_frame_bytes,
    )
    .is_err()
    {
        cancel.store(true, Ordering::SeqCst);
        return Err(WorkerError::Reported);
    }

    let mut request = request.clone();
    cap_request(&mut request, config);
    if let Err(error) = request.validate() {
        send_error(
            output,
            Some(id),
            ErrorCode::BadRequest,
            error.to_string(),
            config.max_frame_bytes,
        );
        return Err(WorkerError::Reported);
    }

    let mut progress_error = false;
    let info = InputInfo {
        total_bytes: Some(source.bytes),
        truncated: source.truncated,
        preview_only: false,
    };
    let result = scan_file_with_info(&path, info, &request, cancel, &mut |progress| {
        if !progress_error {
            if write_message(
                output,
                &WorkerMessage::Progress { id, progress },
                config.max_frame_bytes,
            )
            .is_err()
            {
                progress_error = true;
                cancel.store(true, Ordering::SeqCst);
            }
        }
    });

    match result {
        Ok(report) => {
            if write_message(
                output,
                &WorkerMessage::Result { id, report },
                config.max_frame_bytes,
            )
            .is_err()
            {
                return Err(WorkerError::Reported);
            }
            Ok(())
        }
        Err(ScanError::InvalidRequest(message)) => {
            send_error(
                output,
                Some(id),
                ErrorCode::BadRequest,
                message,
                config.max_frame_bytes,
            );
            Err(WorkerError::Reported)
        }
        Err(ScanError::Io(message)) => {
            send_error(
                output,
                Some(id),
                ErrorCode::Io,
                message,
                config.max_frame_bytes,
            );
            Err(WorkerError::Reported)
        }
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
) {
    let _ = write_message(
        output,
        &WorkerMessage::Error { id, code, message },
        max_frame_bytes,
    );
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
        control.begin(1);
        control.cancel(2);
        assert!(!control.flag.load(Ordering::SeqCst));
        control.cancel(1);
        assert!(control.flag.load(Ordering::SeqCst));
        control.end();
        control.begin(1);
        assert!(
            control.flag.load(Ordering::SeqCst),
            "pending cancel is kept"
        );
        control.end();
        control.begin(1);
        assert!(!control.flag.load(Ordering::SeqCst));
    }
}
