// SPDX-License-Identifier: Apache-2.0
use viewer_shell::browse::{Navigation, Request, Stamp};
use viewer_shell::{Document, Table};
#[test]
fn completion_is_consumed_once_not_just_generation_checked() {
    let mut d = Document::default();
    let g = d.begin();
    assert!(d.finish(
        g,
        Ok(Table {
            coverage: "accepted".into(),
            ..Default::default()
        })
    ));
    assert!(!d.finish(g, Err("late duplicate".into())));
    assert_eq!(d.status, "accepted");
}
#[test]
fn unsolicited_initial_completion_is_rejected() {
    let mut d = Document::default();
    assert!(!d.finish(0, Ok(Table::default())));
    assert!(d.table.is_none());
}
#[test]
fn navigation_does_not_keep_previous_cursors_from_another_source_stamp() {
    let mut n = Navigation::default();
    for (offset, bytes) in [(0, 10), (200, 10), (400, 20)] {
        n.commit(Request {
            offset,
            expected: Some(Stamp {
                bytes,
                modified_ns: Some(bytes),
            }),
            ..Default::default()
        });
    }
    assert!(
        n.previous().is_none(),
        "history must belong to the current source epoch"
    );
}

#[test]
fn cancellation_invalidates_both_old_and_unsolicited_new_generation_results() {
    let mut d = Document::default();
    let old = d.begin();
    d.cancel();
    assert!(!d.finish(old, Ok(Table::default())));
    assert!(!d.finish(d.generation, Ok(Table::default())));
    assert!(d.table.is_none());
    assert_eq!(d.status, "Cancelled.");
}
