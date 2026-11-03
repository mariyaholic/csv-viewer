// SPDX-License-Identifier: Apache-2.0
use std::path::Path;
use viewer_shell::guidance::{self, guide_path, HELP_ITEMS};
#[test]
fn help_topics_are_named_unique_and_have_no_hidden_file_actions() {
    assert_eq!(HELP_ITEMS.len(), 7);
    let mut ids = std::collections::HashSet::new();
    let mut access_keys = std::collections::HashSet::new();
    for (id, label) in HELP_ITEMS {
        assert!(ids.insert(id));
        assert!((119..=125).contains(&id));
        let key = label
            .split_once('&')
            .unwrap()
            .1
            .chars()
            .next()
            .unwrap()
            .to_ascii_lowercase();
        assert!(access_keys.insert(key), "duplicate access key: {key}");
    }
    assert!(HELP_ITEMS
        .iter()
        .any(|(_, s)| s.contains("Offline user guide")));
    assert!(HELP_ITEMS
        .iter()
        .any(|(_, s)| s.contains("Troubleshooting")));
}
#[test]
fn guide_is_anchored_to_the_executable_not_a_source_or_working_directory() {
    let base = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("portable");
    assert_eq!(
        guide_path(&base.join("csv-desktop.exe")).unwrap(),
        base.join("help/index.html")
    );
    assert!(guide_path(Path::new("csv-desktop.exe")).is_none());
}
#[test]
fn help_keeps_limits_recovery_copy_and_credits_in_their_topics() {
    assert!(guidance::HELP.contains("Open file"));
    assert!(guidance::HELP.contains("Made by Maria"));
    assert!(guidance::SEARCH.contains("first 64 columns"));
    assert!(guidance::SEARCH.contains("Next"));
    assert!(guidance::SEARCH.contains("Clear search"));
    assert!(guidance::SEARCH.contains("displayed"));
    assert!(guidance::TROUBLESHOOTING.contains("Reload"));
    assert!(guidance::APPEARANCE.contains("normal close"));
    assert!(guidance::SHORTCUTS.contains("table"));
    assert!(guidance::ABOUT.contains("Apache-2.0"));
    assert!(!guidance::HELP.contains("64 MiB"));
    assert!(guidance::HELP.chars().count() < 900);
}
