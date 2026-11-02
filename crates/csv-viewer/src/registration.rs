// SPDX-License-Identifier: Apache-2.0

//! `--register`/`--unregister`: the visible, confirming CLI path (07A-07).
//!
//! regsvr32 has no console, so the change is only announced here. The
//! decision logic is platform-neutral and seam-tested; the execution call
//! lives behind `cfg(windows)`.

use crate::args::RegistrationAction;

/// Print the warning, read one confirmation line, and run the registration.
/// `execute` is only called after a y/yes answer; anything else cancels with
/// nothing changed. Returns the outcome message for the caller to print.
#[cfg(any(windows, test))]
pub fn confirm_and_run(
    action: RegistrationAction,
    force: bool,
    print: &mut dyn FnMut(&str),
    read_line: &mut dyn FnMut() -> String,
    execute: &mut dyn FnMut(RegistrationAction, bool) -> Result<(), String>,
) -> Result<(), String> {
    print(csv_preview::registration::REGISTRATION_WARNING);
    print(if action == RegistrationAction::Register {
        "Type y and press Enter to continue, or anything else to cancel."
    } else {
        "Unregistration removes only values that still point to this handler. \
         Type y and press Enter to continue, or anything else to cancel."
    });
    let answer = read_line().trim().to_ascii_lowercase();
    if !matches!(answer.as_str(), "y" | "yes") {
        print("Cancelled; nothing was changed.");
        return Ok(());
    }
    execute(action, force)
}

/// The real CLI flow with console I/O. Returns the process exit code.
#[cfg(windows)]
pub fn cli(action: RegistrationAction, force: bool) -> i32 {
    let mut print = |line: &str| println!("{line}");
    let mut read_line = || {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        line
    };
    let mut execute = |action: RegistrationAction, force: bool| -> Result<(), String> {
        let status = match action {
            RegistrationAction::Register => csv_preview::windows::register_command(force),
            RegistrationAction::Unregister => csv_preview::windows::unregister_command(),
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "the {} attempt failed (0x{:08X})",
                action.label(),
                status as u32
            ))
        }
    };
    match confirm_and_run(action, force, &mut print, &mut read_line, &mut execute) {
        Ok(()) => {
            println!("csv-viewer: done.");
            0
        }
        Err(message) => {
            eprintln!("csv-viewer: {message}");
            1
        }
    }
}

/// On non-Windows builds the registration and its DLL do not exist.
#[cfg(not(windows))]
pub fn cli(action: RegistrationAction, _force: bool) -> i32 {
    eprintln!(
        "csv-viewer: --{} requires Windows; the preview handler is a Windows component",
        action.label()
    );
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::RegistrationAction;

    #[test]
    fn a_declined_confirmation_never_executes() {
        let mut lines: Vec<String> = Vec::new();
        let mut print = |line: &str| lines.push(line.to_string());
        let mut read_line = || "no".to_string();
        let mut executed: Vec<(RegistrationAction, bool)> = Vec::new();
        let mut execute = |action: RegistrationAction, force: bool| -> Result<(), String> {
            executed.push((action, force));
            Ok(())
        };
        let outcome = confirm_and_run(
            RegistrationAction::Register,
            false,
            &mut print,
            &mut read_line,
            &mut execute,
        );
        assert!(outcome.is_ok());
        assert!(
            executed.is_empty(),
            "nothing may run without a y/yes answer"
        );
        assert!(lines.iter().any(|line| line.contains("Type y")));
    }

    #[test]
    fn a_confirmed_confirmation_executes_with_the_force_flag() {
        let mut lines: Vec<String> = Vec::new();
        let mut print = |line: &str| lines.push(line.to_string());
        let mut read_line = || "y".to_string();
        let mut executed: Vec<(RegistrationAction, bool)> = Vec::new();
        let mut execute = |action: RegistrationAction, force: bool| -> Result<(), String> {
            executed.push((action, force));
            Ok(())
        };
        let outcome = confirm_and_run(
            RegistrationAction::Unregister,
            true,
            &mut print,
            &mut read_line,
            &mut execute,
        );
        assert!(outcome.is_ok());
        assert_eq!(executed, vec![(RegistrationAction::Unregister, true)]);
        assert!(lines
            .iter()
            .any(|line| line.contains("make CSV viewer and preview the preview handler")));
    }

    #[test]
    fn executed_errors_surface() {
        let mut print = |_: &str| {};
        let mut read_line = || "YE S".to_string(); // not y/yes → cancel
        let mut executed: Vec<(RegistrationAction, bool)> = Vec::new();
        let mut execute = |action: RegistrationAction, force: bool| -> Result<(), String> {
            executed.push((action, force));
            Err("failed".to_string())
        };
        let outcome = confirm_and_run(
            RegistrationAction::Register,
            true,
            &mut print,
            &mut read_line,
            &mut execute,
        );
        assert!(outcome.is_ok(), "a lowered non-y answer still cancels");
        assert!(executed.is_empty());
    }

    #[test]
    fn executed_errors_surface_yes() {
        let mut print = |_: &str| {};
        let mut read_line = || "yes".to_string();
        let mut executed: Vec<(RegistrationAction, bool)> = Vec::new();
        let mut execute = |action: RegistrationAction, force: bool| -> Result<(), String> {
            executed.push((action, force));
            Err("failed".to_string())
        };
        let outcome = confirm_and_run(
            RegistrationAction::Register,
            true,
            &mut print,
            &mut read_line,
            &mut execute,
        );
        assert_eq!(outcome, Err("failed".to_string()));
        assert_eq!(executed, vec![(RegistrationAction::Register, true)]);
    }
}
