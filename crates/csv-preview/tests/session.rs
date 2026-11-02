// SPDX-License-Identifier: Apache-2.0

use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicBool, AtomicUsize};
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
    let preview = snapshot
        .preview
        .expect("preview should be available immediately");
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
    let session = PreviewSession::start(Cursor::new(csv), Some(100), &request, config).unwrap();
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

/// 07A-09: a source that fills the spool cap exactly is fully spooled, not
/// truncated. Complementary to `spool_cap_marks_a_scan_incomplete`, which
/// covers the over-cap case.
#[test]
fn spool_exact_cap_is_not_truncated() {
    // The 90-byte source fits the spool exactly: head takes 10 of the 90
    // bytes, the remaining 80 fill the rest of the 90-byte cap.
    let csv = vec![b'x'; 90];
    let request = ScanRequest::full();
    let mut config = config();
    config.head_bytes = 10;
    config.max_spool_bytes = 90;
    let session = PreviewSession::start(Cursor::new(csv), Some(90), &request, config).unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::SpoolReady);
    assert!(
        !snapshot.spool.truncated,
        "exact cap is complete, not truncated"
    );
    assert_eq!(snapshot.spool.bytes, 90);
    assert_eq!(snapshot.spool.total_bytes, Some(90));
    assert!(!snapshot.spool_source.expect("spool").truncated);

    session
        .start_full_scan(Arc::new(InProcessBackend), request, Duration::from_secs(5))
        .unwrap();
    assert!(session.wait_for_scan(Duration::from_secs(5)));
    let full = session.snapshot().full_report.expect("full report");
    assert!(
        full.complete,
        "an exactly cap-sized source must scan complete"
    );
    full.validate_invariants().unwrap();
}

/// 07A-09: the cap+1 production shape (a source one byte past the cap) must
/// still be reported as truncated — the extra byte is what the probe reads.
#[test]
fn spool_one_byte_past_the_cap_is_truncated() {
    // One byte past the 90-byte cap, with the +1 byte available for the
    // truncation probe (the shape copy_owned_source feeds in production).
    let csv = vec![b'x'; 91];
    let request = ScanRequest::full();
    let mut config = config();
    config.head_bytes = 10;
    config.max_spool_bytes = 90;
    let session = PreviewSession::start(Cursor::new(csv), Some(91), &request, config).unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::SpoolReady);
    assert!(
        snapshot.spool.truncated,
        "one byte past the cap is truncated"
    );
    assert_eq!(snapshot.spool.bytes, 90);
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

/// 07A-12: the worker integration runs only when explicitly requested
/// (`cargo test -- --ignored`, which CI does after building the workspace).
/// A missing binary can no longer hide behind a green skip: an explicit run
/// without the worker fails.
#[test]
#[ignore = "needs csv-worker built; run explicitly with -- --ignored (CI does, after cargo build --workspace)"]
fn worker_backend_matches_in_process_report() {
    let Some(worker) = worker_beside_test_executable() else {
        if std::env::var_os("CI").is_some() {
            panic!("CI is missing csv-worker.exe; build the workspace before the tests");
        }
        panic!(
            "worker integration requires csv-worker.exe; build the workspace first (cargo build --workspace)"
        );
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
    assert_eq!(
        snapshot.phase,
        SessionPhase::Complete,
        "{:?}",
        snapshot.error
    );
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
    // 07A-12: production composes `read_head` on the host apartment and hands
    // the rest of the stream to the session through `start_with_head`
    // (07A-02). Test that composition, not the plain `start` shortcut that
    // production never takes.
    let mut csv = b"a,b\n".to_vec();
    for index in 0..50 {
        csv.extend_from_slice(format!("{index},value\n").as_bytes());
    }
    let mut config = config();
    config.head_bytes = 32;
    let request = ScanRequest::full();
    let mut reader = SlowReader {
        data: csv.clone(),
        position: 0,
    };
    let head = csv_preview::session::read_head(&mut reader, config.head_bytes).unwrap();
    assert_eq!(head.len(), 32);
    let session = PreviewSession::start_with_head(head, reader, None, &request, config).unwrap();
    assert!(session.snapshot().preview.is_some());
    // The slow remainder is spooled completely on the session thread.
    assert!(session.wait_for_spool(Duration::from_secs(10)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::SpoolReady);
    // The spool file holds the head plus everything after it: the whole
    // source. (bytes_read is unchanged - the head was read from the same
    // single stream once.)
    assert_eq!(
        snapshot.spool.bytes,
        csv.len() as u64,
        "the rest of the slow stream must be spooled completely"
    );
    assert!(!snapshot.spool.truncated);
}

/// 07A-12: exactly one copy of the source flows from the reader into the
/// spool. A duplicate copy (a spooled tempfile being spooled again) or a
/// re-read would show up as a byte count above the source size.
#[test]
fn the_source_flows_into_the_spool_exactly_once() {
    #[derive(Default)]
    struct CountingReader {
        data: Vec<u8>,
        position: usize,
        zero_reads: Arc<AtomicUsize>,
        bytes_read: Arc<AtomicUsize>,
    }

    impl Read for CountingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.position >= self.data.len() {
                self.zero_reads
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Ok(0);
            }
            let take = buffer.len().min(self.data.len() - self.position);
            buffer[..take].copy_from_slice(&self.data[self.position..self.position + take]);
            self.position += take;
            self.bytes_read
                .fetch_add(take, std::sync::atomic::Ordering::SeqCst);
            Ok(take)
        }
    }

    let data = CSV.to_vec();
    let mut config = config();
    config.head_bytes = 16;
    let request = ScanRequest::full();
    let bytes_read = Arc::new(AtomicUsize::new(0));
    let zero_reads = Arc::new(AtomicUsize::new(0));
    let reader = CountingReader {
        data: data.clone(),
        position: 0,
        zero_reads: Arc::clone(&zero_reads),
        bytes_read: Arc::clone(&bytes_read),
    };
    let mut reader = reader;
    let head = csv_preview::session::read_head(&mut reader, config.head_bytes).unwrap();
    assert_eq!(head.len(), 16);
    let head_bytes = bytes_read.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(head_bytes, 16, "the head comes from the same single stream");
    let session = PreviewSession::start_with_head(head, reader, None, &request, config).unwrap();
    assert!(session.wait_for_spool(Duration::from_secs(5)));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.phase, SessionPhase::SpoolReady);
    assert!(!snapshot.spool.truncated);
    let expected = data.len() - 16;
    assert_eq!(
        bytes_read.load(std::sync::atomic::Ordering::SeqCst),
        head_bytes + expected,
        "every byte must be read exactly once - the head on the calling \
         thread and the rest once by the spool thread"
    );
    // The spool file contains the head too (write_spool writes it first),
    // so the full source size must appear as the spooled byte count.
    assert_eq!(
        snapshot.spool.bytes,
        data.len() as u64,
        "the spooled byte count must match the read byte count"
    );
    assert!(
        zero_reads.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "the spool must have detected the end of the source"
    );
}

/// A source that keeps producing slowly. A whole-stream copy would not return
/// for seconds, so a head-only pipeline must return while almost all of the
/// source is still unread.
struct NeverReader {
    remaining: u64,
}

impl Read for NeverReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || buffer.is_empty() {
            return Ok(0);
        }
        std::thread::sleep(Duration::from_millis(1));
        let take = buffer.len().min(self.remaining as usize);
        buffer[..take].fill(b'x');
        self.remaining -= take as u64;
        Ok(take)
    }
}

#[test]
fn head_only_stream_preview_returns_promptly() {
    // 256 MiB of slow source: the old eager full-stream copy could not return
    // within a second; reading only the head returns in milliseconds.
    let mut reader = NeverReader {
        remaining: 256 * 1024 * 1024,
    };
    let request = ScanRequest::full();
    let config = PreviewConfig::default();
    let started = Instant::now();
    let head = csv_preview::session::read_head(&mut reader, config.head_bytes).unwrap();
    assert_eq!(head.len(), config.head_bytes);
    let session = PreviewSession::start_with_head(head, reader, None, &request, config).unwrap();
    let elapsed = started.elapsed();
    assert!(
        session.snapshot().preview.is_some(),
        "the immediate preview must be published before the spool finishes"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the head-only pipeline took {elapsed:?}"
    );
    session.cancel();
    assert!(session.wait_for_spool(Duration::from_secs(2)));
}
