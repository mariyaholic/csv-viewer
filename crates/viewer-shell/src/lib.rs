// SPDX-License-Identifier: Apache-2.0
//! Pure standalone viewer state and theme contract. No COM dependency.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub background: u32,
    pub panel: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub info: u32,
}
pub const THEMES: &[Theme] = &[
    Theme {
        id: "paper",
        name: "Paper",
        background: 0xf2ede2,
        panel: 0xfffdf8,
        text: 0x241f18,
        muted: 0x6f665b,
        accent: 0x8a6a20,
        info: 0x6f5faa,
    },
    Theme {
        id: "ink",
        name: "Ink",
        background: 0x60509,
        panel: 0xb0913,
        text: 0xe9e4da,
        muted: 0xaba3ad,
        accent: 0xd8b878,
        info: 0xb9ace9,
    },
    Theme {
        id: "pc98",
        name: "PC-98",
        background: 0x80b22,
        panel: 0x10152e,
        text: 0xeeeadd,
        muted: 0xb2b8d5,
        accent: 0xd8b878,
        info: 0xbdb1ed,
    },
    Theme {
        id: "doloris",
        name: "Doloris",
        background: 0x60509,
        panel: 0xb0913,
        text: 0xe9e4da,
        muted: 0x97908f,
        accent: 0xd8b878,
        info: 0xa497d6,
    },
    Theme {
        id: "sumimi",
        name: "Sumimi",
        background: 0xf2ede2,
        panel: 0xfffdf8,
        text: 0x241f18,
        muted: 0x6f665b,
        accent: 0x8a6a20,
        info: 0x6f5faa,
    },
    Theme {
        id: "marigold",
        name: "Marigold",
        background: 0xb0913,
        panel: 0x14101c,
        text: 0xe9e4da,
        muted: 0x97908f,
        accent: 0xd8b878,
        info: 0xa497d6,
    },
];
pub fn theme(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub coverage: String,
    pub details: String,
}
impl Table {
    pub fn bounded(mut self) -> Self {
        let source_columns = self.columns.len();
        let source_rows = self.rows.len();
        self.columns.truncate(64);
        self.rows.truncate(200);
        let mut budget = 8 * 1024 * 1024;
        for col in &mut self.columns {
            *col = display_text(col, 256);
        }
        let mut kept = 0;
        for row in &mut self.rows {
            row.truncate(self.columns.len());
            let mut cost = 0;
            for value in row {
                *value = display_text(value, 4096);
                cost += value.len();
            }
            if cost > budget {
                break;
            }
            budget -= cost;
            kept += 1;
        }
        self.rows.truncate(kept);
        if source_columns > self.columns.len() || source_rows > self.rows.len() {
            self.coverage = format!(
                "{}; display limited to {} rows / {} columns",
                self.coverage,
                self.rows.len(),
                self.columns.len()
            );
        }
        self.details = self
            .details
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
            .take(128 * 1024)
            .collect();
        let mut end = self.details.len().min(128 * 1024);
        while !self.details.is_char_boundary(end) {
            end -= 1;
        }
        self.details.truncate(end);
        self.coverage = display_text(&self.coverage, 2048);
        self
    }
}
/// Escape control characters, bound UTF-8 bytes and make truncation explicit.
pub fn display_text(s: &str, limit: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let part = if c.is_control() {
            c.escape_default().to_string()
        } else {
            c.to_string()
        };
        if out.len() + part.len() > limit.saturating_sub(3) {
            if limit >= 3 {
                out.push('…');
            }
            break;
        }
        out.push_str(&part);
    }
    out
}
pub mod supervisor;
#[cfg(windows)]
pub mod win32;
#[derive(Default)]
pub struct Document {
    pub generation: u64,
    pub table: Option<Table>,
    pub status: String,
}
impl Document {
    pub fn begin(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.status = "Opening: transfer / inspect (up to 120 s)".into();
        self.generation
    }
    pub fn finish(&mut self, generation: u64, result: Result<Table, String>) -> bool {
        if generation != self.generation {
            return false;
        }
        match result {
            Ok(t) => {
                let t = t.bounded();
                self.status = t.coverage.clone();
                self.table = Some(t)
            }
            Err(e) => self.status = e,
        };
        true
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn metadata_keeps_line_breaks_but_cells_escape_them() {
        let t = super::Table {
            details: "Schema\n  field".into(),
            columns: vec!["x".into()],
            rows: vec![vec!["a\nb".into()]],
            ..Default::default()
        }
        .bounded();
        assert!(t.details.contains('\n'));
        assert!(!t.rows[0][0].contains('\n'));
    }

    use super::*;
    #[test]
    fn six_unique_designs_and_safe_fallback() {
        assert_eq!(THEMES.len(), 6);
        let mut ids = std::collections::BTreeSet::new();
        for t in THEMES {
            assert!(ids.insert(t.id));
        }
        assert_eq!(theme("bad").id, "paper");
    }
    #[test]
    fn stale_result_cannot_replace_a_new_document() {
        let mut d = Document::default();
        let a = d.begin();
        let b = d.begin();
        assert_ne!(a, b);
        assert!(!d.finish(a, Err("stale error".into())));
        assert!(d.finish(
            b,
            Ok(Table {
                coverage: "current".into(),
                ..Table::default()
            })
        ));
        assert_eq!(d.status, "current");
    }
    #[test]
    fn failure_keeps_loaded_preview() {
        let mut d = Document::default();
        let a = d.begin();
        d.finish(a, Ok(Table::default()));
        let b = d.begin();
        d.finish(b, Err("worker unavailable".into()));
        assert!(d.table.is_some());
        assert_eq!(d.status, "worker unavailable");
    }
    #[test]
    fn display_has_row_column_and_byte_budgets() {
        let t = Table {
            columns: vec!["x".repeat(500); 500],
            rows: vec![vec!["z".repeat(9000); 100]; 201],
            ..Table::default()
        }
        .bounded();
        assert!(t.rows.len() <= 200);
        assert!(t.columns.len() <= 64);
        assert!(t.rows.iter().flatten().all(|s| s.len() <= 4096));
        assert!(t.rows.iter().flatten().map(String::len).sum::<usize>() <= 8 * 1024 * 1024);
    }
}
