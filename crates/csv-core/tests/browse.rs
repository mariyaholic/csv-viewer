// SPDX-License-Identifier: Apache-2.0
use csvview_core::scan::browse_page;
use std::io::{Cursor, Write};
#[test]
fn pages_preserve_source_rows_multiline_fields_and_leading_zeros() {
    let source = b"account,note\n001,\"first\nline\"\n002,second\n003,third\n";
    let first = browse_page(Cursor::new(source), 0, 2, "").unwrap();
    assert_eq!(first.rows[0], vec!["001", "first\nline"]);
    assert_eq!(first.next, Some(2));
    let second = browse_page(Cursor::new(source), 2, 2, "").unwrap();
    assert_eq!(second.rows[0][0], "003");
    assert_eq!(second.source_rows, vec![2]);
    assert_eq!(second.next, None);
}
#[test]
fn search_reaches_beyond_first_page_and_uses_literal_text() {
    let source = b"id,note\n001,alpha\n002,beta\n003,NEEDLE.*\n004,needle.*\n";
    let page = browse_page(Cursor::new(source), 0, 1, "needle.*").unwrap();
    assert_eq!(page.rows[0][0], "003");
    assert_eq!(page.source_rows, vec![2]);
    let next = browse_page(Cursor::new(source), page.next.unwrap(), 2, "needle.*").unwrap();
    assert_eq!(next.rows[0][0], "004");
    assert_eq!(next.next, None);
}
#[test]
fn large_source_does_not_require_full_read_or_spool() {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(b"id,note\n001,alpha\n002,beta\n003,gamma\n")
        .unwrap();
    file.set_len(80 * 1024 * 1024).unwrap();
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).unwrap();
    let page = browse_page(file, 0, 2, "").unwrap();
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.next, Some(2));
}
#[test]
fn empty_search_at_eof_is_explicit_and_record_memory_is_bounded() {
    let page = browse_page(Cursor::new(b"id,note\n001,alpha\n"), 0, 200, "missing").unwrap();
    assert!(page.rows.is_empty());
    assert_eq!(page.next, None);
    let hostile = format!("id,note\n{}\n", ",".repeat(2 * 1024 * 1024));
    assert!(browse_page(Cursor::new(hostile), 0, 200, "").is_err());
}

#[test]
fn utf8_checkpoints_resume_bom_multiline_and_literal_bom_values_without_replaying() {
    let bytes =
        "\u{feff}id,note\r\n001,\"a\nb\"\r\n002,plain\r\n003,\u{feff}still-data\r\n".as_bytes();
    let a = browse_page(Cursor::new(bytes), 0, 2, "").unwrap();
    let byte = a.next_byte.unwrap();
    // csv may leave the LF of CRLF for its next read; both positions are
    // valid resumable record boundaries, not necessarily the next value byte.
    assert!(byte > 0 && matches!(bytes[byte as usize - 1], b'\r' | b'\n'));
    let b = csvview_core::scan::browse_page_at(Cursor::new(bytes), 2, 2, "", Some(byte)).unwrap();
    assert_eq!(b.rows, vec![vec!["003", "\u{feff}still-data"]]);
    assert_eq!(b.source_rows, vec![2]);
    assert!(b.next.is_none());
}

#[test]
fn utf8_bom_cannot_hide_an_unclosed_first_quoted_record_from_guard() {
    assert!(browse_page(Cursor::new(b"\xef\xbb\xbf\"unterminated\n"), 0, 200, "").is_err());
}

#[test]
fn non_utf8_pages_replay_decoded_rows_without_mistaking_positions_for_source_bytes() {
    let text = "id;note\n001;café\n002;BETA\n003;tail\n";
    let bytes: Vec<u8> = [0xff, 0xfe]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let page = browse_page(Cursor::new(&bytes), 1, 1, "beta").unwrap();
    assert_eq!(page.rows, vec![vec!["002", "BETA"]]);
    assert_eq!(page.source_rows, vec![1]);
    assert!(page.next_byte.is_none());
    assert!(csvview_core::scan::browse_page_at(Cursor::new(&bytes), 1, 1, "", Some(20)).is_err());
    let page = browse_page(Cursor::new(b"id;note\n001;caf\xe9\n"), 0, 200, "CAFÉ").unwrap();
    assert_eq!(page.source_rows, vec![0]);
    assert!(page.next_byte.is_none());
}
#[test]
fn checkpoint_at_lf_record_start_does_not_strip_a_real_bom_value() {
    let bytes = "id,note\n001,plain\n002,plain\n003,plain\n\u{feff}004,second\n".as_bytes();
    let first = browse_page(Cursor::new(bytes), 0, 3, "").unwrap();
    let second =
        csvview_core::scan::browse_page_at(Cursor::new(bytes), 3, 1, "", first.next_byte).unwrap();
    assert_eq!(second.rows, vec![vec!["\u{feff}004", "second"]]);
}

#[test]
fn late_utf8_checkpoint_reads_only_header_and_current_window() {
    use std::io::{Read, Seek, SeekFrom};
    struct Count {
        inner: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for Count {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(b)?;
            self.read += n;
            Ok(n)
        }
    }
    impl Seek for Count {
        fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(p)
        }
    }
    let mut text = "id,note\n".to_owned();
    let mut byte = 0;
    for i in 0..100_000 {
        if i == 90_000 {
            byte = text.len() as u64;
        }
        text.push_str(&format!("{i:06},ordinary\n"));
    }
    let mut source = Count {
        inner: Cursor::new(text.into_bytes()),
        read: 0,
    };
    let page =
        csvview_core::scan::browse_page_at(&mut source, 90_000, 200, "", Some(byte)).unwrap();
    assert_eq!(page.rows[0][0], "090000");
    assert_eq!(page.source_rows[0], 90_000);
    assert!(
        source.read < 70 * 1024,
        "late page replayed prefix: {} bytes",
        source.read
    );
}
