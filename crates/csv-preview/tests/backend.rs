// SPDX-License-Identifier: Apache-2.0

//! Smoke test for the worker-backed backend spawn path (07A-03).
//!
//! The window-suppression attribute on the Windows spawn is not observable
//! in-process, so the automated gate is behavioural: a full worker scan still
//! completes over piped stdio with the attribute set. Windows-only console
//! flashing additionally needs the manual check in docs/TESTING.md.

use std::io::Write;
use std::sync::atomic::AtomicBool;

use csv_preview::{worker_beside_test_executable, FullScanBackend, SpoolSource, WorkerBackend};
use csvview_core::ScanRequest;

const CSV: &[u8] = b"name,value\n007,1.5\n008,=SUM(A1)\n009,true\n";

#[test]
fn worker_scan_over_piped_stdio_returns_a_report() {
    let Some(worker) = worker_beside_test_executable() else {
        eprintln!("skipping worker integration: build csv-worker first");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.csv");
    std::fs::File::create(&source)
        .unwrap()
        .write_all(CSV)
        .unwrap();
    let spool = SpoolSource {
        path: source,
        bytes: CSV.len() as u64,
        truncated: false,
    };
    let backend = WorkerBackend::new(worker);
    let report = backend
        .scan(
            &spool,
            &ScanRequest::full(),
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .expect("a full scan must complete over piped stdio");
    assert_eq!(report.total_records, Some(3), "{:?}", report.warnings);
}
