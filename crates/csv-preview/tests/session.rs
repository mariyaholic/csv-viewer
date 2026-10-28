// SPDX-License-Identifier: Apache-2.0

use std::io::{Cursor, Read};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use csv_preview::{
    worker_beside_test_executable, FullScanBackend, InProcessBackend, PreviewConfig,
    PreviewSession, SessionPhase, SpoolSource, WorkerBackend,
};
use csvview_core::{ScanProgress, ScanReport, ScanRequest};

const CSV: &[u8] = b"name,value\n007,1.5\n008,=SUM(A1)\n009,true\n";

fn config() -> PreviewConfig {
    PreviewConfig {
        head_bytes: 64,
        max_spool_bytes: 1024 * 1024,
        preview_rows: 2,
        spool_chunk_bytes: 8,
        scan_timeout: Duration::from_secs(5),
    }
}

#[test]
fn preview_is_immediate_and_full_scan_is_explicit() {
    let request = ScanRequest::full();
    let session = PreviewSession::start(
        Cursor::new(CSV.to_vec()),
        Some(CSV.len() as u64),
        &request,
        config(),
    )
    .unwrap();
    let snapshot = session.snapshot();
    let preview = snapshot.preview.expect("preview should be available immediately");
    assert!(!preview.complete, "a head preview is never a complete scan");
    assert_eq!(preview.total_records, None);
    assert!(!preview.preview_rows.is_empty());
    assert!(preview.preview_rows.len() <= 2);

    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::SpoolReady);
    assert_eq!(snapshot.spool.bytes, CSV.len() as u64);
    assert!(!snapshot.spool.truncated);
    assert!(snapshot.spool_source.is_some());

    session
        .start_full_scan(Arc::new(InProcessBackend), request, Duration::from_secs(5))
        .unwrap();
    assert!(session.wait_for_scan(Duration::from_secs(5)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::Complete);
    let full = snapshot.full_report.expect("full report");
    assert!(full.complete);
    assert_eq!(full.total_records, Some(3));
    full.validate_invariants().unwrap();
}

#[test]
fn spool_cap_marks_a_scan_incomplete() {
    let csv = vec![b'x'; 100];
    let request = ScanRequest::full();
    let mut config = config();
    config.max_spool_bytes = 10;
    let session =
        PreviewSession::start(Cursor::new(csv), Some(100), &request, config).unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let snapshot = session.snapshot();
    assert!(snapshot.spool.truncated);
    assert_eq!(snapshot.spool.bytes, 10);

    session
        .start_full_scan(Arc::new(InProcessBackend), request, Duration::from_secs(5))
        .unwrap();
    assert!(session.wait_for_scan(Duration::from_secs(5)));
    let full = session.snapshot().full_report.expect("full report");
    assert!(!full.complete);
    assert_eq!(full.total_records, None);
    assert!(full.scanned.source_truncated);
}

#[test]
fn cancellation_is_terminal() {
    let request = ScanRequest::full();
    let session = PreviewSession::start(
        Cursor::new(CSV.to_vec()),
        Some(CSV.len() as u64),
        &request,
        config(),
    )
    .unwrap();
    session.cancel();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::Cancelled);
    assert!(snapshot.full_report.is_none());
}

#[test]
fn timeout_does_not_publish_a_report() {
    struct SlowBackend;
    impl FullScanBackend for SlowBackend {
        fn scan(
            &self,
            _spool: &SpoolSource,
            _request: &ScanRequest,
            _cancel: &AtomicBool,
            _progress: &mut dyn FnMut(ScanProgress),
        ) -> Result<ScanReport, String> {
            std::thread::sleep(Duration::from_millis(200));
            Err("late".to_string())
        }
    }

    let request = ScanRequest::full();
    let session = PreviewSession::start(
        Cursor::new(CSV.to_vec()),
        Some(CSV.len() as u64),
        &request,
        config(),
    )
    .unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    session
        .start_full_scan(Arc::new(SlowBackend), request, Duration::from_millis(20))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if session.snapshot().phase == SessionPhase::TimedOut {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::TimedOut);
    assert!(snapshot.full_report.is_none());
}

#[test]
fn worker_backend_matches_in_process_report() {
    let Some(worker) = worker_beside_test_executable() else {
        eprintln!("skipping worker integration: build csv-worker first");
        return;
    };
    let request = ScanRequest::full();
    let session = PreviewSession::start(
        Cursor::new(CSV.to_vec()),
        Some(CSV.len() as u64),
        &request,
        config(),
    )
    .unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let worker = WorkerBackend {
        stop_grace: Duration::from_millis(200),
        ..WorkerBackend::new(worker)
    };
    session
        .start_full_scan(Arc::new(worker), request.clone(), Duration::from_secs(10))
        .unwrap();
    assert!(session.wait_for_scan(Duration::from_secs(10)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::Complete, "{:?}", snapshot.error);
    let remote = snapshot.full_report.expect("remote report");
    let local = csvview_core::scan(
        Cursor::new(CSV.to_vec()),
        csvview_core::InputInfo::known(CSV.len() as u64),
        &request,
    )
    .unwrap();
    assert_eq!(*remote, local);
}

#[test]
fn missing_worker_is_reported() {
    let request = ScanRequest::full();
    let session = PreviewSession::start(
        Cursor::new(CSV.to_vec()),
        Some(CSV.len() as u64),
        &request,
        config(),
    )
    .unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let worker = WorkerBackend::new("/definitely/not/a/worker");
    session
        .start_full_scan(Arc::new(worker), request, Duration::from_secs(2))
        .unwrap();
    assert!(session.wait_for_scan(Duration::from_secs(2)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::Failed);
    assert!(snapshot.error.unwrap().contains("worker not found"));
}

struct SlowReader {
    data: Vec<u8>,
    position: usize,
}

impl Read for SlowReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.data.len() {
            return Ok(0);
        }
        std::thread::sleep(Duration::from_millis(2));
        let take = buffer.len().min(8).min(self.data.len() - self.position);
        buffer[..take].copy_from_slice(&self.data[self.position..self.position + take]);
        self.position += take;
        Ok(take)
    }
}

#[test]
fn slow_stream_preview_still_returns_a_head() {
    let mut csv = b"a,b\n".to_vec();
    for index in 0..50 {
        csv.extend_from_slice(format!("{index},value\n").as_bytes());
    }
    let mut config = config();
    config.head_bytes = 32;
    let request = ScanRequest::full();
    let session = PreviewSession::start(
        SlowReader {
            data: csv,
            position: 0,
        },
        None,
        &request,
        config,
    )
    .unwrap();
    assert!(session.snapshot().preview.is_some());
}
