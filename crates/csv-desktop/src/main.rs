// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    let adapter = csv_desktop::adapter();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|s| s == "--check") {
        let Some(path) = args.get(1) else {
            std::process::exit(2);
        };
        let result = viewer_shell::supervisor::worker_path(adapter).and_then(|worker| {
            viewer_shell::supervisor::Task::start(adapter, path.into(), worker).wait()
        });
        match result {
            Ok(t) => {
                println!(
                    "{}\n{}\n{}",
                    t.coverage,
                    t.columns.join(" | "),
                    t.rows
                        .iter()
                        .map(|r| r.join(" | "))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if let Err(e) = viewer_shell::win32::run(adapter, args) {
        viewer_shell::win32::error(&e);
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("Native GUI requires Windows. --check <file> exercises the same worker adapter.");
        std::process::exit(2);
    }
}
