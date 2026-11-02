// SPDX-License-Identifier: Apache-2.0
use csv_protocol::{
    read_message, write_message, ClientMessage, SourceSpec, WorkerMessage, PROTOCOL_VERSION,
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};
use viewer_shell::{
    supervisor::{copy_cancelled, Adapter},
    Table,
};
pub fn adapter() -> Adapter {
    Adapter {
        title: "CSV Viewer",
        worker: "csv-worker",
        csv_spool: true,
        inspect,
    }
}
fn inspect(
    file: &mut File,
    spool: &Path,
    mut input: &mut dyn Write,
    mut output: &mut dyn Read,
    cancel: &AtomicBool,
) -> Result<Table, String> {
    let cap = csv_protocol::frame::MAX_FRAME_BYTES;
    write_message(
        &mut input,
        &ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
        cap,
    )
    .map_err(|e| e.to_string())?;
    match read_message::<_, WorkerMessage>(&mut output, cap).map_err(|e| e.to_string())? {
        WorkerMessage::Ready {
            protocol_version, ..
        } if protocol_version == PROTOCOL_VERSION => {}
        other => return Err(format!("Worker handshake rejected: {other:?}")),
    }
    let path = spool.join("source.csv");
    let mut dest = File::create(&path).map_err(|e| e.to_string())?;
    let bytes = copy_cancelled(file, &mut dest, cancel)?;
    drop(dest);
    let request = csvview_core::ScanRequest::preview();
    write_message(
        &mut input,
        &ClientMessage::Scan {
            id: 1,
            source: SourceSpec {
                path: path.to_str().ok_or("Spool path is not Unicode")?.into(),
                bytes,
                truncated: false,
            },
            request,
        },
        cap,
    )
    .map_err(|e| e.to_string())?;
    loop {
        match read_message::<_, WorkerMessage>(&mut output, cap).map_err(|e| e.to_string())? {
            WorkerMessage::Result { id: 1, report } => return table(report),
            WorkerMessage::Error { message, .. } => return Err(message),
            WorkerMessage::Accepted { id: 1 } | WorkerMessage::Progress { id: 1, .. } => {}
            _ => return Err("Unexpected CSV worker response".into()),
        }
    }
}
fn table(report: csvview_core::ScanReport) -> Result<Table, String> {
    report.validate_invariants()?;
    let coverage = format!(
        "{} retained sample rows; {}",
        report.preview_rows.len(),
        report.coverage_summary()
    );
    let details = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    Ok(Table {
        columns: report.columns.iter().map(|c| c.name.clone()).collect(),
        rows: report.preview_rows.into_iter().map(|r| r.cells).collect(),
        coverage,
        details,
    }
    .bounded())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_text_and_coverage_survive_adapter() {
        let report = csvview_core::scan(
            std::io::Cursor::new(b"account,amount\n001024,-42.00\n"),
            csvview_core::InputInfo::known(29),
            &csvview_core::ScanRequest::preview(),
        )
        .unwrap();
        let t = table(report).unwrap();
        assert_eq!(t.rows[0][0], "001024");
        assert!(t.coverage.contains("retained sample"));
    }
}
