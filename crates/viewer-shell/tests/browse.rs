// SPDX-License-Identifier: Apache-2.0
use viewer_shell::browse::{Navigation, Request};
#[test]
fn previous_tracks_successful_pages_not_a_guessed_row_count() {
    let mut n = Navigation::default();
    n.commit(Request {
        offset: 200,
        ..Request::default()
    });
    n.commit(Request {
        offset: 480,
        query: String::new(),
        ..Request::default()
    });
    assert_eq!(n.previous().unwrap().offset, 200);
    n.commit(n.previous().unwrap());
    assert_eq!(n.previous().unwrap().offset, 0);
    n.commit(Request {
        offset: 0,
        query: "needle".into(),
        ..Request::default()
    });
    assert!(n.previous().is_none());
}
#[test]
fn request_rejects_unbounded_search_and_history_is_bounded() {
    assert!(Request {
        query: "x".repeat(1025),
        ..Request::default()
    }
    .validate()
    .is_err());
    let mut n = Navigation::default();
    for offset in 1..1000 {
        n.commit(Request {
            offset,
            ..Request::default()
        });
    }
    assert!(n.history_len() <= 256);
}

#[test]
fn framing_rejects_truncated_and_oversized_inputs_before_allocating_body() {
    use std::io::Cursor;
    let mut bytes = Vec::new();
    viewer_shell::browse::write(&mut bytes, &Request::default()).unwrap();
    let decoded: Request = viewer_shell::browse::read(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(decoded, Request::default());
    assert!(
        viewer_shell::browse::read::<Request>(&mut Cursor::new(&bytes[..bytes.len() - 1])).is_err()
    );
    assert!(
        viewer_shell::browse::read::<Request>(&mut Cursor::new(u32::MAX.to_le_bytes()))
            .unwrap_err()
            .contains("exceeds")
    );
    let request = Request {
        byte_offset: Some(10),
        ..Default::default()
    };
    assert!(request.validate().is_err());
}

#[test]
fn successful_restart_clears_old_cursors_instead_of_pushing_a_later_page() {
    let mut n = Navigation::default();
    let request = Request {
        query: "needle".into(),
        ..Default::default()
    };
    n.commit(request.clone());
    n.commit(Request {
        offset: 200,
        ..request.clone()
    });
    n.commit(Request {
        offset: 400,
        ..request.clone()
    });
    assert_eq!(n.current.offset, 400);
    // A failed/cancelled operation never calls commit: current/history stay intact.
    assert_eq!(n.previous().unwrap().offset, 200);
    n.commit(request);
    assert!(n.previous().is_none());
    assert_eq!(n.current.offset, 0);
}
