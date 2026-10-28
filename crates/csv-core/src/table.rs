// SPDX-License-Identifier: Apache-2.0

//! Virtualized table window used by the companion viewer.
//!
//! Only the visible window is rendered; the full preview is kept as source
//! text and never converted.

use std::ops::Range;

use crate::model::ScanReport;

#[derive(Debug, Clone, Default)]
pub struct TableModel {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    row_offset: usize,
    column_offset: usize,
    viewport_rows: usize,
    viewport_columns: usize,
}

impl TableModel {
    pub fn new(columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self {
            columns,
            rows,
            row_offset: 0,
            column_offset: 0,
            viewport_rows: 1,
            viewport_columns: 1,
        }
    }

    pub fn from_report(report: &ScanReport) -> Self {
        let columns = report.columns.iter().map(|c| c.name.clone()).collect();
        let rows = report
            .preview_rows
            .iter()
            .map(|row| row.cells.clone())
            .collect();
        Self::new(columns, rows)
    }

    pub fn set_viewport(&mut self, rows: usize, columns: usize) {
        self.viewport_rows = rows.max(1);
        self.viewport_columns = columns.max(1);
        self.clamp();
    }

    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn column_offset(&self) -> usize {
        self.column_offset
    }

    pub fn viewport_rows(&self) -> usize {
        self.viewport_rows
    }

    pub fn viewport_columns(&self) -> usize {
        self.viewport_columns
    }

    pub fn visible_rows(&self) -> Range<usize> {
        let start = self.row_offset.min(self.rows.len());
        let end = (start + self.viewport_rows).min(self.rows.len());
        start..end
    }

    pub fn visible_columns(&self) -> Range<usize> {
        let start = self.column_offset.min(self.columns.len());
        let end = (start + self.viewport_columns).min(self.columns.len());
        start..end
    }

    pub fn scroll_rows(&mut self, delta: isize) {
        self.row_offset = offset(self.row_offset, delta, self.rows.len());
        self.clamp();
    }

    pub fn scroll_columns(&mut self, delta: isize) {
        self.column_offset = offset(self.column_offset, delta, self.columns.len());
        self.clamp();
    }

    pub fn page_rows(&mut self, direction: isize) {
        let amount = self.viewport_rows.saturating_sub(1).max(1) as isize * direction;
        self.scroll_rows(amount);
    }

    pub fn home(&mut self) {
        self.row_offset = 0;
        self.column_offset = 0;
    }

    pub fn end(&mut self) {
        self.row_offset = self.rows.len().saturating_sub(self.viewport_rows);
        self.column_offset = self.columns.len().saturating_sub(self.viewport_columns);
    }

    pub fn cell(&self, row: usize, column: usize) -> Option<&str> {
        self.rows
            .get(row)
            .and_then(|cells| cells.get(column))
            .map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() || self.columns.is_empty()
    }

    fn clamp(&mut self) {
        self.row_offset = self
            .row_offset
            .min(self.rows.len().saturating_sub(self.viewport_rows));
        self.column_offset = self
            .column_offset
            .min(self.columns.len().saturating_sub(self.viewport_columns));
    }
}

fn offset(current: usize, delta: isize, length: usize) -> usize {
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current
            .saturating_add(delta as usize)
            .min(length.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> TableModel {
        TableModel::new(
            vec!["a".into(), "b".into(), "c".into()],
            (0..10)
                .map(|row| vec![format!("{row}a"), format!("{row}b"), format!("{row}c")])
                .collect(),
        )
    }

    #[test]
    fn viewport_is_bounded() {
        let mut table = table();
        table.set_viewport(4, 2);
        assert_eq!(table.visible_rows(), 0..4);
        assert_eq!(table.visible_columns(), 0..2);
        table.scroll_rows(100);
        assert_eq!(table.visible_rows(), 6..10);
        table.scroll_columns(100);
        assert_eq!(table.visible_columns(), 1..3);
    }

    #[test]
    fn paging_keeps_one_row_of_context() {
        let mut table = table();
        table.set_viewport(4, 3);
        table.page_rows(1);
        assert_eq!(table.row_offset(), 3);
        table.page_rows(-1);
        assert_eq!(table.row_offset(), 0);
    }

    #[test]
    fn end_and_home_are_clamped() {
        let mut table = table();
        table.set_viewport(3, 2);
        table.end();
        assert_eq!(table.row_offset(), 7);
        assert_eq!(table.column_offset(), 1);
        table.home();
        assert_eq!(table.row_offset(), 0);
        assert_eq!(table.column_offset(), 0);
    }

    #[test]
    fn empty_table_is_safe() {
        let mut table = TableModel::new(Vec::new(), Vec::new());
        table.set_viewport(5, 5);
        table.scroll_rows(10);
        table.scroll_columns(-10);
        assert!(table.visible_rows().is_empty());
        assert!(table.is_empty());
    }
}
