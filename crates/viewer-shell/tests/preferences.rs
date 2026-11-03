// SPDX-License-Identifier: Apache-2.0
use viewer_shell::preferences::{load, save, Area, Preferences, Window};

#[test]
fn missing_settings_start_with_paper_and_no_stored_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let p = load(&dir.path().join("missing.json")).unwrap();
    assert_eq!(p.theme, "paper");
    assert!(p.window.is_none());
}

#[test]
fn size_position_theme_and_maximized_state_round_trip_without_document_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/desktop.json");
    let p = Preferences {
        theme: "doloris".into(),
        window: Some(Window {
            x: -1100,
            y: 45,
            width: 1050,
            height: 650,
            dpi: 144,
            maximized: true,
        }),
        ..Default::default()
    };
    save(&path, &p).unwrap();
    assert_eq!(load(&path).unwrap(), p);
    let json = std::fs::read_to_string(path).unwrap();
    assert!(!json.contains("source"));
    assert!(!json.contains("query"));
    assert!(!json.contains("path"));
}

#[test]
fn replacement_is_atomic_and_leaves_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    for theme in ["paper", "ink", "pc98", "doloris", "sumimi", "marigold"] {
        let p = Preferences {
            theme: theme.into(),
            ..Default::default()
        };
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap().theme, theme);
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn damaged_oversized_or_future_settings_are_rejected_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    for contents in [
        "{broken".to_owned(),
        " ".repeat(17000),
        "{\"version\":2,\"theme\":\"ink\"}".into(),
    ] {
        std::fs::write(&path, &contents).unwrap();
        assert!(load(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
    }
}

#[test]
fn invalid_theme_and_geometry_use_safe_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    std::fs::write(&path, r#"{"theme":"missing","window":{"x":0,"y":0,"width":0,"height":0,"dpi":0,"maximized":false}}"#).unwrap();
    let p = load(&path).unwrap();
    assert_eq!(p.theme, "paper");
    assert!(p.window.is_none());
}

#[test]
fn disconnected_monitor_placement_is_visible_and_scaled_for_current_dpi() {
    let saved = Window {
        x: -4000,
        y: -2000,
        width: 1000,
        height: 600,
        dpi: 96,
        maximized: false,
    };
    let fitted = saved.fit(
        Area {
            left: 0,
            top: 40,
            right: 1920,
            bottom: 1040,
        },
        144,
    );
    assert_eq!(
        (fitted.x, fitted.y, fitted.width, fitted.height),
        (0, 40, 1500, 900)
    );
    assert_eq!(fitted.dpi, 144);
}

#[test]
fn negative_monitor_coordinates_are_preserved_and_small_workspaces_are_bounded() {
    let saved = Window {
        x: -1100,
        y: 50,
        width: 1000,
        height: 600,
        dpi: 96,
        maximized: true,
    };
    assert_eq!(
        saved.fit(
            Area {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1080
            },
            96
        ),
        saved
    );
    let fit = saved.fit(
        Area {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        },
        192,
    );
    assert_eq!((fit.x, fit.y, fit.width, fit.height), (0, 0, 800, 600));
    assert!(fit.maximized);
}

#[test]
fn invalid_values_cannot_overflow_placement_math() {
    let saved = Window {
        x: i32::MIN,
        y: i32::MAX,
        width: u32::MAX,
        height: u32::MAX,
        dpi: 0,
        maximized: false,
    };
    let fit = saved.fit(
        Area {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        },
        0,
    );
    assert!(fit.width <= 1024 && fit.height <= 768);
    assert!(fit.x >= 0 && fit.y >= 0);
}

#[test]
fn save_failure_does_not_damage_existing_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    save(&path, &Preferences::default()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let future = Preferences {
        version: 999,
        ..Default::default()
    };
    assert!(save(&path, &future).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(save(
        &dir.path().join("desktop.json/child.json"),
        &Preferences::default()
    )
    .is_err());
}

#[test]
fn viewers_keep_separate_paths_and_reject_unknown_application_names() {
    use viewer_shell::preferences::file_in;
    let root = std::path::Path::new("appearance");
    assert_ne!(file_in(root, "CSV Viewer"), file_in(root, "Parquet Viewer"));
    assert!(file_in(root, "../other").is_none());
    assert!(file_in(std::path::Path::new(""), "CSV Viewer").is_none());
}

#[test]
fn concurrent_closes_produce_one_complete_settings_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = ["paper", "ink", "pc98", "doloris"]
        .into_iter()
        .map(|theme| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..25 {
                    save(
                        &path,
                        &Preferences {
                            theme: theme.into(),
                            ..Default::default()
                        },
                    )
                    .map_err(|e| format!("concurrent replacement failed: {e}"))?;
                }
                Ok::<_, String>(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    assert!(["paper", "ink", "pc98", "doloris"].contains(&load(&path).unwrap().theme.as_str()));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(windows)]
#[test]
fn locked_destination_reports_failure_without_deleting_existing_settings() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("desktop.json");
    save(&path, &Preferences::default()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    assert!(save(
        &path,
        &Preferences {
            theme: "ink".into(),
            ..Default::default()
        }
    )
    .is_err());
    drop(lock);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
