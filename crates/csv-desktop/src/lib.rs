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
        page: None,
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

#[cfg(test)]
mod browse_tests {
    use super::*;
    #[test]
    fn desktop_page_has_real_source_offsets_and_detects_stale_sources() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/csv-core/tests/fixtures/golden.csv");
        let request = viewer_shell::browse::Request {
            offset: 1,
            ..Default::default()
        };
        let page = browse(&path, &request).unwrap();
        assert_eq!(page.page.as_ref().unwrap().source_rows[0], 1);
        let mut request = viewer_shell::browse::Request {
            query: "definitely absent literal.*".into(),
            ..Default::default()
        };
        let missing = browse(&path, &request).unwrap();
        assert!(missing.rows.is_empty());
        assert!(missing.page.unwrap().next.is_none());
        request.expected = Some(viewer_shell::browse::Stamp {
            bytes: 0,
            modified_ns: None,
        });
        assert!(browse(&path, &request).unwrap_err().contains("changed"));
    }
}

/// Desktop-only direct read, invoked in an owned child, never the UI thread.
pub fn browse(path: &Path, request: &viewer_shell::browse::Request) -> Result<Table, String> {
    request.validate()?;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    let stamp = viewer_shell::browse::Stamp::from_metadata(&meta);
    if !meta.is_file() {
        return Err("Choose a regular local file".into());
    }
    if request.expected.as_ref().is_some_and(|s| *s != stamp) {
        return Err("Source changed; Reload before continuing".into());
    }

    if stamp.bytes > 4 * 1024 * 1024 * 1024 {
        return Err("CSV desktop file limit: 4 GiB".into());
    }
    if request.byte_offset.is_some_and(|b| b > meta.len()) {
        return Err("CSV checkpoint is outside the file".into());
    }
    let page = csvview_core::scan::browse_page_at(
        &mut file,
        request.offset,
        200,
        &request.query,
        request.byte_offset,
    )
    .map_err(|e| e.to_string())?;
    if viewer_shell::browse::Stamp::from_metadata(&file.metadata().map_err(|e| e.to_string())?)
        != stamp
    {
        return Err("Source changed while reading; Reload".into());
    }
    let coverage=format!("{} page rows; scanned {} rows from {}; {}. Literal lowercase matching in first 64 columns.",page.rows.len(),page.scanned_to.saturating_sub(request.offset),request.offset.saturating_add(1),if page.next.is_some(){"Next continues reading/search"}else{"end of source reached"});
    let details = format!(
        "{} 20 s per parse; 1 MiB per record / 16 KiB per field; 1 MiB retained raw text.",
        page.notes
    );
    Ok(Table {
        columns: page.columns,
        rows: page.rows,
        coverage,
        details,
        page: Some(viewer_shell::browse::Page {
            start: request.offset,
            next: page.next,
            source_rows: page.source_rows,
            stamp,
            next_byte: page.next_byte,
            scanned_to: page.scanned_to,
        }),
    }
    .bounded())
}
