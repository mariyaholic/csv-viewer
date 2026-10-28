// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use csvview_core::{
    scan_file, ScanLimits, ScanReport, ScanRequest, TableModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    FullScan,
}

pub struct App {
    pub report: ScanReport,
    pub table: TableModel,
    pub path: PathBuf,
    pub request: ScanRequest,
    pub show_warnings: bool,
    pub show_about: bool,
    pub show_details: bool,
    pub warning_scroll: usize,
    pub scanning: bool,
    pub message: Option<String>,
    receiver: Option<Receiver<Result<ScanReport, String>>>,
}

impl App {
    pub fn new(report: ScanReport, path: PathBuf, request: ScanRequest) -> Self {
        let table = TableModel::from_report(&report);
        Self {
            report,
            table,
            path,
            request,
            show_warnings: false,
            show_about: false,
            show_details: false,
            warning_scroll: 0,
            scanning: false,
            message: None,
            receiver: None,
        }
    }

    pub fn set_viewport(&mut self, rows: usize, columns: usize) {
        self.table.set_viewport(rows, columns);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc => {
                if self.show_about || self.show_warnings || self.show_details {
                    self.show_about = false;
                    self.show_warnings = false;
                    self.show_details = false;
                    Action::None
                } else {
                    Action::Quit
                }
            }
            KeyCode::Char('a') => {
                self.show_about = !self.show_about;
                if self.show_about {
                    self.show_warnings = false;
                    self.show_details = false;
                }
                Action::None
            }
            KeyCode::Char('w') => {
                self.show_warnings = !self.show_warnings;
                if self.show_warnings {
                    self.show_about = false;
                    self.show_details = false;
                }
                Action::None
            }
            KeyCode::Char('d') => {
                self.show_details = !self.show_details;
                if self.show_details {
                    self.show_about = false;
                    self.show_warnings = false;
                }
                Action::None
            }
            KeyCode::Char('f') if !self.report.complete && !self.scanning => Action::FullScan,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.show_warnings {
                    self.warning_scroll = self.warning_scroll.saturating_sub(1);
                } else {
                    self.table.scroll_rows(-1);
                }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.show_warnings {
                    self.warning_scroll = self.warning_scroll.saturating_add(1);
                } else {
                    self.table.scroll_rows(1);
                }
                Action::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.table.scroll_columns(-1);
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.table.scroll_columns(1);
                Action::None
            }
            KeyCode::PageUp => {
                self.table.page_rows(-1);
                Action::None
            }
            KeyCode::PageDown => {
                self.table.page_rows(1);
                Action::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.table.home();
                self.warning_scroll = 0;
                Action::None
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.table.end();
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn start_full_scan(&mut self) {
        if self.scanning || self.report.complete {
            return;
        }
        let path = self.path.clone();
        let request = self.full_scan_request();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.scanning = true;
        self.message = Some("full scan running…".to_string());
        if let Err(error) = std::thread::Builder::new()
            .name("csv-viewer-scan".to_string())
            .spawn(move || {
                let result = scan_file(&path, &request).map_err(|error| error.to_string());
                let _ = sender.send(result);
            })
        {
            self.scanning = false;
            self.receiver = None;
            self.message = Some(format!("could not start full scan: {error}"));
        }
    }

    pub fn poll_full_scan(&mut self) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };
        match receiver.try_recv() {
            Ok(Ok(report)) => {
                self.message = Some(report.coverage_summary());
                self.report = report;
                self.table = TableModel::from_report(&self.report);
                self.scanning = false;
                self.receiver = None;
                self.warning_scroll = 0;
                true
            }
            Ok(Err(error)) => {
                self.message = Some(format!("full scan failed: {error}"));
                self.scanning = false;
                self.receiver = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.message = Some("full scan stopped without a result".to_string());
                self.scanning = false;
                self.receiver = None;
                true
            }
        }
    }

    pub fn full_scan_request(&self) -> ScanRequest {
        let mut request = self.request.clone();
        request.limits = ScanLimits::full();
        request.preview_rows = 100;
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csvview_core::{scan, InputInfo};
    use std::io;

    fn app() -> App {
        let report = scan(
            io::Cursor::new(b"a,b\n1,2\n3,4\n".to_vec()),
            InputInfo::known(12),
            &ScanRequest::full(),
        )
        .unwrap();
        App::new(report, PathBuf::from("fixture.csv"), ScanRequest::full())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn scrolls_and_opens_overlays() {
        let mut app = app();
        let rows = app.report.preview_rows.len();
        assert!(rows > 0);
        app.handle_key(key(KeyCode::Char('w')));
        assert!(app.show_warnings);
        app.handle_key(key(KeyCode::Char('a')));
        assert!(app.show_about);
        assert!(!app.show_warnings);
        app.handle_key(key(KeyCode::Char('d')));
        assert!(app.show_details);
        assert!(!app.show_about);
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_about);
    }

    #[test]
    fn quit_and_full_scan_actions() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Action::Quit);
        let mut preview_app = {
            let report = scan(
                io::Cursor::new(b"a,b\n1,2\n".to_vec()),
                InputInfo::preview(Some(8)),
                &ScanRequest::preview(),
            )
            .unwrap();
            App::new(report, PathBuf::from("fixture.csv"), ScanRequest::preview())
        };
        assert_eq!(
            preview_app.handle_key(key(KeyCode::Char('f'))),
            Action::FullScan
        );
        assert_eq!(
            preview_app.full_scan_request().limits.max_records,
            ScanLimits::full().max_records
        );
    }

    #[test]
    fn table_scroll_is_bounded() {
        let mut app = app();
        app.set_viewport(1, 1);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::End));
        assert_eq!(app.table.row_offset(), app.table.rows.len() - 1);
        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.table.row_offset(), 0);
    }
}
