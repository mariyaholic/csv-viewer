// SPDX-License-Identifier: Apache-2.0

use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use csv_protocol::{
    read_message, write_message, ClientMessage, ErrorCode, SourceSpec, WorkerMessage,
    PROTOCOL_VERSION,
};
use csvview_core::{IncompleteReason, InputInfo, ScanLimits, ScanRequest};
use tempfile::TempDir;

struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Worker {
    fn start(spool: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_csv-worker"))
            .arg("--spool-dir")
            .arg(spool)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("worker should start");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut worker = Self {
            child,
            stdin,
            stdout,
        };
        worker.send(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
        });
        match worker.receive() {
            WorkerMessage::Ready {
                protocol_version,
                max_frame_bytes,
                ..
            } => {
                assert_eq!(protocol_version, PROTOCOL_VERSION);
                assert!(max_frame_bytes >= 1024);
            }
            other => panic!("expected ready, got {other:?}"),
        }
        worker
    }

    fn send(&mut self, message: &ClientMessage) {
        write_message(&mut self.stdin, message, 16 * 1024 * 1024).unwrap();
    }

    fn receive(&mut self) -> WorkerMessage {
        read_message(&mut self.stdout, 16 * 1024 * 1024).unwrap()
    }

    fn shutdown(mut self) {
        self.send(&ClientMessage::Shutdown);
        let _ = read_message::<_, WorkerMessage>(&mut self.stdout, 16 * 1024 * 1024);
        let _ = self.child.wait();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn full_request() -> ScanRequest {
    let mut request = ScanRequest::full();
    request.limits = ScanLimits {
        max_records: Some(1000),
        max_bytes: Some(1024 * 1024),
        deadline_ms: Some(10_000),
        ..ScanLimits::full()
    };
    request
}

fn write_spool(directory: &TempDir, name: &str, bytes: &[u8]) -> SourceSpec {
    let path = directory.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    SourceSpec {
        path: path.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
        truncated: false,
    }
}

#[test]
fn scans_a_spool_file_end_to_end() {
    let spool = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(spool.path());
    let source = write_spool(&spool, "scan.csv", b"a,b\n1,2\n3,4\n");
    worker.send(&ClientMessage::Scan {
        id: 1,
        source,
        request: full_request(),
    });
    assert_eq!(worker.receive(), WorkerMessage::Accepted { id: 1 });
    let report = loop {
        match worker.receive() {
            WorkerMessage::Progress { id, .. } => assert_eq!(id, 1),
            WorkerMessage::Result { id, report } => {
                assert_eq!(id, 1);
                break report;
            }
            other => panic!("unexpected message {other:?}"),
        }
    };
    assert!(report.complete);
    assert_eq!(report.total_records, Some(2));
    assert_eq!(report.columns.len(), 2);
    report.validate_invariants().unwrap();
    worker.shutdown();
}

#[test]
fn truncated_source_is_reported_as_partial() {
    let spool = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(spool.path());
    let mut source = write_spool(&spool, "truncated.csv", b"a,b\n1,2\n");
    source.truncated = true;
    worker.send(&ClientMessage::Scan {
        id: 2,
        source,
        request: full_request(),
    });
    assert_eq!(worker.receive(), WorkerMessage::Accepted { id: 2 });
    let report = loop {
        match worker.receive() {
            WorkerMessage::Progress { .. } => {}
            WorkerMessage::Result { report, .. } => break report,
            other => panic!("unexpected message {other:?}"),
        }
    };
    assert!(!report.complete);
    assert_eq!(report.total_records, None);
    assert!(report.scanned.source_truncated);
    worker.shutdown();
}

#[test]
fn rejects_a_path_outside_the_spool() {
    let spool = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("secret.csv");
    std::fs::write(&outside_file, b"a,b\n").unwrap();
    let mut worker = Worker::start(spool.path());
    worker.send(&ClientMessage::Scan {
        id: 3,
        source: SourceSpec {
            path: outside_file.to_string_lossy().into_owned(),
            bytes: 4,
            truncated: false,
        },
        request: full_request(),
    });
    match worker.receive() {
        WorkerMessage::Error { id, code, .. } => {
            assert_eq!(id, Some(3));
            assert_eq!(code, ErrorCode::SourceOutsideSpool);
        }
        other => panic!("expected an error, got {other:?}"),
    }
    worker.shutdown();
}

#[test]
fn rejects_a_size_mismatch() {
    let spool = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(spool.path());
    let mut source = write_spool(&spool, "mismatch.csv", b"a,b\n");
    source.bytes += 1;
    worker.send(&ClientMessage::Scan {
        id: 4,
        source,
        request: full_request(),
    });
    match worker.receive() {
        WorkerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::SourceSizeMismatch),
        other => panic!("expected an error, got {other:?}"),
    }
    worker.shutdown();
}

#[test]
fn rejects_a_wrong_protocol_version() {
    let spool = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_csv-worker"))
        .arg("--spool-dir")
        .arg(spool.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    write_message(
        &mut stdin,
        &ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION + 1,
        },
        16 * 1024 * 1024,
    )
    .unwrap();
    match read_message::<_, WorkerMessage>(&mut stdout, 16 * 1024 * 1024).unwrap() {
        WorkerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::BadVersion),
        other => panic!("expected bad version, got {other:?}"),
    }
    let _ = child.wait();
}

#[test]
fn report_matches_in_process_scan() {
    let spool = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(spool.path());
    let bytes = b"date,value\n2024-01-02,007\n2024-01-01,=1+1\n";
    let source = write_spool(&spool, "compare.csv", bytes);
    worker.send(&ClientMessage::Scan {
        id: 5,
        source,
        request: full_request(),
    });
    assert_eq!(worker.receive(), WorkerMessage::Accepted { id: 5 });
    let remote = loop {
        match worker.receive() {
            WorkerMessage::Progress { .. } => {}
            WorkerMessage::Result { report, .. } => break report,
            other => panic!("unexpected {other:?}"),
        }
    };
    let local = csvview_core::scan(
        std::io::Cursor::new(bytes.to_vec()),
        InputInfo::known(bytes.len() as u64),
        &full_request(),
    )
    .unwrap();
    assert_eq!(remote, local);
    worker.shutdown();
}

/// One record whose single field is far beyond `max_field_chars * 4`: the
/// reader must stop inside the record instead of buffering the rest of the
/// source whole (07A-04). Peak worker memory is measured manually on this
/// PC; see docs/TESTING.md.
#[test]
fn worker_budget_reports_partial_with_huge_field() {
    let spool = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(spool.path());
    let path = spool.path().join("huge_field.csv");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"a,b\n1,\"").unwrap();
    let chunk = [b'a'; 1 << 20];
    for _ in 0..12 {
        file.write_all(&chunk).unwrap();
    }
    file.write_all(b"\"\n").unwrap();
    let bytes = std::fs::metadata(&path).unwrap().len();
    let mut request = ScanRequest::full();
    request.limits = ScanLimits {
        // Large enough that only the field budget, not the byte limit,
        // could stop this scan.
        max_bytes: Some(64 * 1024 * 1024),
        deadline_ms: Some(10_000),
        ..ScanLimits::full()
    };
    let started = Instant::now();
    worker.send(&ClientMessage::Scan {
        id: 6,
        source: SourceSpec {
            path: path.to_string_lossy().into_owned(),
            bytes,
            truncated: false,
        },
        request,
    });
    assert_eq!(worker.receive(), WorkerMessage::Accepted { id: 6 });
    let report = loop {
        match worker.receive() {
            WorkerMessage::Progress { .. } => {}
            WorkerMessage::Result { id, report } => {
                assert_eq!(id, 6);
                break report;
            }
            other => panic!("unexpected message {other:?}"),
        }
    };
    // Well under the 10 s deadline: the scan stops near the budget, not at
    // the end of the source.
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "took {:?}",
        started.elapsed()
    );
    assert!(!report.complete);
    assert_eq!(report.incomplete_reason, Some(IncompleteReason::ParseError));
    assert!(report
        .parse_error
        .as_deref()
        .is_some_and(|error| error.contains("per-field byte budget")));
    assert_eq!(report.scanned.data_records, 0);
    report.validate_invariants().unwrap();
    worker.shutdown();
}

#[allow(dead_code)]
fn read_stderr(mut child: Child) -> String {
    let mut stderr = String::new();
    if let Some(mut handle) = child.stderr.take() {
        let _ = handle.read_to_string(&mut stderr);
    }
    stderr
}
