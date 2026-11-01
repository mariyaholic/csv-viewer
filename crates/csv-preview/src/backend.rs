// SPDX-License-Identifier: Apache-2.0

//! Worker-backed full scan.
//!
//! The worker is a separate, version-matched process. It receives a path that
//! must be inside the private spool directory passed at startup, plus an exact
//! byte count. This module owns the child process and kills it on cancellation,
//! timeout, error or drop; a job object is not a security sandbox, so the
//! worker also enforces its own byte/record/time caps.

use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use csv_protocol::{
    read_message, write_message, ClientMessage, ErrorCode, FrameError, SourceSpec, WorkerMessage,
    PROTOCOL_VERSION,
};
use csvview_core::{ScanProgress, ScanReport, ScanRequest};

use crate::{FullScanBackend, SpoolSource};

/// CREATE_NO_WINDOW: the child never gets its own console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct WorkerBackend {
    pub worker_path: PathBuf,
    pub max_frame_bytes: usize,
    pub handshake_timeout: Duration,
    pub stop_grace: Duration,
}

impl WorkerBackend {
    pub fn new(worker_path: impl Into<PathBuf>) -> Self {
        Self {
            worker_path: worker_path.into(),
            max_frame_bytes: csv_protocol::frame::MAX_FRAME_BYTES,
            handshake_timeout: Duration::from_secs(10),
            stop_grace: Duration::from_millis(500),
        }
    }

    /// Find the worker next to the current executable, or via
    /// `CSV_VIEWER_WORKER`. Paths are local only.
    pub fn discover() -> Result<Self, String> {
        if let Some(path) = std::env::var_os("CSV_VIEWER_WORKER") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Ok(Self::new(path));
            }
            return Err(format!(
                "CSV_VIEWER_WORKER does not point to a file: {}",
                path.display()
            ));
        }
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let directory = executable
            .parent()
            .ok_or_else(|| "current executable has no parent directory".to_string())?;
        for name in ["csv-worker.exe", "csv-worker"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Ok(Self::new(candidate));
            }
        }
        Err(format!(
            "csv-worker was not found next to {}; set CSV_VIEWER_WORKER",
            executable.display()
        ))
    }

    fn scan_inner(
        &self,
        spool: &SpoolSource,
        request: &ScanRequest,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, String> {
        if !self.worker_path.is_file() {
            return Err(format!(
                "worker not found at {}",
                self.worker_path.display()
            ));
        }
        let spool_dir = spool
            .path
            .parent()
            .ok_or_else(|| "spool path has no parent directory".to_string())?;
        let mut command = Command::new(&self.worker_path);
        command
            .arg("--spool-dir")
            .arg(spool_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            // The worker is a console-subsystem executable and the preview
            // host (prevhost.exe) has no console: without this attribute
            // Windows allocates and flashes a console window for every full
            // scan. Piped stdio is unaffected by CREATE_NO_WINDOW.
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start worker: {error}"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "worker stdin was not available".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "worker stdout was not available".to_string())?;
        let stderr = child.stderr.take();

        let (sender, receiver) = mpsc::channel();
        let max_frame = self.max_frame_bytes;
        thread::Builder::new()
            .name("csv-worker-reader".to_string())
            .spawn(move || {
                let _alive = crate::lifecycle::LiveThread::enter();
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_message::<_, WorkerMessage>(&mut reader, max_frame) {
                        Ok(message) => {
                            if sender.send(Ok(message)).is_err() {
                                break;
                            }
                        }
                        Err(FrameError::Closed) => break,
                        Err(error) => {
                            let _ = sender.send(Err(error.to_string()));
                            break;
                        }
                    }
                }
            })
            .map_err(|error| format!("cannot start worker reader: {error}"))?;

        let overall = Instant::now()
            + Duration::from_millis(
                request
                    .limits
                    .deadline_ms
                    .unwrap_or(120_000)
                    .saturating_add(5_000),
            );
        let handshake_deadline = (Instant::now() + self.handshake_timeout).min(overall);

        let result = (|| -> Result<ScanReport, String> {
            write_client(
                &mut stdin,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                },
                self.max_frame_bytes,
            )?;
            match receive(&receiver, handshake_deadline, cancel)? {
                Some(WorkerMessage::Ready {
                    protocol_version,
                    max_frame_bytes,
                    ..
                }) => {
                    if protocol_version != PROTOCOL_VERSION {
                        return Err(format!(
                            "worker protocol {protocol_version} does not match {PROTOCOL_VERSION}"
                        ));
                    }
                    if max_frame_bytes < 1024 {
                        return Err("worker reported an invalid frame limit".to_string());
                    }
                }
                Some(WorkerMessage::Error { code, message, .. }) => {
                    return Err(format!("worker handshake failed ({code:?}): {message}"));
                }
                Some(other) => {
                    return Err(format!("unexpected worker handshake message: {other:?}"))
                }
                None => return Err("worker handshake timed out".to_string()),
            }

            write_client(
                &mut stdin,
                &ClientMessage::Scan {
                    id: 1,
                    source: SourceSpec {
                        path: spool
                            .path
                            .to_str()
                            .ok_or_else(|| "spool path is not valid UTF-8".to_string())?
                            .to_string(),
                        bytes: spool.bytes,
                        truncated: spool.truncated,
                    },
                    request: request.clone(),
                },
                self.max_frame_bytes,
            )?;

            loop {
                match receive(&receiver, overall, cancel)? {
                    Some(WorkerMessage::Accepted { id }) => {
                        if id != 1 {
                            return Err(format!("worker accepted unexpected scan id {id}"));
                        }
                    }
                    Some(WorkerMessage::Progress {
                        id,
                        progress: update,
                    }) => {
                        if id != 1 {
                            return Err(format!("worker progress for unexpected scan id {id}"));
                        }
                        progress(update);
                    }
                    Some(WorkerMessage::Result { id, report }) => {
                        if id != 1 {
                            return Err(format!("worker result for unexpected scan id {id}"));
                        }
                        report.validate_invariants().map_err(|message| {
                            format!("worker returned an invalid report: {message}")
                        })?;
                        return Ok(report);
                    }
                    Some(WorkerMessage::Error { code, message, .. }) => {
                        let suffix = if code == ErrorCode::Cancelled {
                            " (cancelled)"
                        } else {
                            ""
                        };
                        return Err(format!("worker error {code:?}{suffix}: {message}"));
                    }
                    Some(WorkerMessage::Goodbye) => {
                        return Err("worker closed before returning a result".to_string());
                    }
                    Some(WorkerMessage::Ready { .. }) => {
                        return Err("worker sent a second ready message".to_string());
                    }
                    None => return Err("worker timed out".to_string()),
                }
            }
        })();

        if cancel.load(Ordering::SeqCst) {
            let _ = write_client(
                &mut stdin,
                &ClientMessage::Cancel { id: 1 },
                self.max_frame_bytes,
            );
        }
        let _ = write_client(&mut stdin, &ClientMessage::Shutdown, self.max_frame_bytes);
        finish_child(&mut child, stderr, self.stop_grace);
        result
    }
}

impl FullScanBackend for WorkerBackend {
    fn scan(
        &self,
        spool: &SpoolSource,
        request: &ScanRequest,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, String> {
        self.scan_inner(spool, request, cancel, progress)
    }
}

fn write_client(
    stdin: &mut ChildStdin,
    message: &ClientMessage,
    max_frame: usize,
) -> Result<(), String> {
    write_message(stdin, message, max_frame)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn receive(
    receiver: &mpsc::Receiver<Result<WorkerMessage, String>>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Option<WorkerMessage>, String> {
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".to_string());
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        let wait = (deadline - now).min(Duration::from_millis(100));
        match receiver.recv_timeout(wait) {
            Ok(Ok(message)) => return Ok(Some(message)),
            Ok(Err(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("worker output closed".to_string())
            }
        }
    }
}

fn finish_child(child: &mut Child, stderr: Option<ChildStderr>, grace: Duration) {
    let deadline = Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    if let Some(mut stderr) = stderr {
        let mut message = String::new();
        let _ = stderr.read_to_string(&mut message);
        if !message.trim().is_empty() {
            eprintln!("csv-worker stderr: {}", message.trim());
        }
    }
}

/// The worker path next to a test executable (`target/<profile>/deps/...`).
pub fn worker_beside_test_executable() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?.parent()?;
    for name in ["csv-worker.exe", "csv-worker"] {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
