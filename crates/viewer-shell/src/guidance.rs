// SPDX-License-Identifier: Apache-2.0
//! Functional guidance only: retain the backend's coverage and limits separately.
pub fn empty(csv: bool) -> &'static str {
    if csv {
        "Open a CSV file\r\n\r\nChoose Open file… or drop one CSV, TSV or text file here.\r\n\r\nFiles are read-only. Help explains search and copying rows."
    } else {
        "Open a Parquet file\r\n\r\nChoose Open file… or drop one Parquet file here.\r\n\r\nFiles are read-only. Help explains search and copying rows."
    }
}
pub fn no_matches(more: bool) -> &'static str {
    if more {
        "No matches in the rows checked. Choose Next to keep searching."
    } else {
        "No matches in the searchable columns of the checked range. Choose Clear search to browse rows."
    }
}
/// Task-sized help dialogs; the full offline guide is shipped beside the app.
pub const HELP_ITEMS: [(u16, &str); 7] = [
    (119, "&Quick help\tF1"),
    (120, "&Offline user guide"),
    (121, "&Browsing, search and copying"),
    (122, "&Appearance and settings"),
    (123, "&Keyboard shortcuts"),
    (124, "&Troubleshooting"),
    (125, "Abo&ut"),
];
pub fn guide_path(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    if !exe.is_absolute() {
        return None;
    }
    exe.parent().map(|p| p.join("help").join("index.html"))
}
pub const HELP: &str = "OPEN\nChoose Open file… or drop one file here. Your file stays read-only.\n\nBROWSE\nPrevious and Next move between pages. Find searches literal text, ignoring case, in the first 64 columns. Next continues an unfinished search; Clear search removes the filter.\n\nINSPECT\nSelect a row for details. Copy shown row copies displayed values, not the full source record.\n\nMORE HELP\nThe Help menu has task-specific help and an offline user guide. F1 opens this quick reference.\n\nMade by Maria — free and open source (Apache-2.0).";
pub const SEARCH: &str = "BROWSE\nPrevious and Next move between pages of up to 200 rows. Previous revisits successful pages, not a fixed row subtraction.\n\nSEARCH\nType Search text, then choose Find or press Enter in the search field. Find starts at the beginning. Matching ignores letter case and treats symbols literally: needle.* is text, not a pattern.\n\nOnly the first 64 columns are searched. A checked range is not necessarily the whole file. If Next is available after no matches, choose it to continue. Parquet searches bounded rendered values; unrendered or shortened tails are not searchable.\n\nClear search returns to the first unfiltered page.\n\nCOPY\nSelect a row, then choose Copy shown row. Only displayed values are copied; … means shortened text. Ctrl+C copies a row only with table focus. Use your data tool for complete original values. Pasting into a spreadsheet may interpret untrusted text as formulas.";
pub const APPEARANCE: &str = "Choose a Theme: Paper, Ink, PC-98, Doloris, Sumimi or Marigold. Move, resize or maximize the window, then use normal close to remember it. CSV and Parquet keep separate preferences.\n\nA minimized window reopens normally. Saved bounds fit the current display; real multi-monitor/DPI acceptance is still pending. With several windows, the last successfully saved appearance wins. Forced termination does not save changes.\n\nOnly appearance is stored: no file paths, searches, source values or clipboard text. Files never reopen automatically.\n\nIf saved appearance cannot be read, the app uses defaults and leaves it untouched. Press F1 for a reset prompt: No preserves it; Yes resets only size/theme. Close normally after the reset. If saving fails, the previous settings are preserved and the app reports it.";
pub const SHORTCUTS: &str = "Ctrl+O — Open file\nCtrl+F — Focus and select Search text\nEnter in Search text — Find from the beginning\nF5 — Reload from the beginning with the applied search\nEsc — Cancel an active read\nF1 — Quick help\nTab / Shift+Tab — Move between controls\nSpace / Enter on a button — Activate that button\nAlt + an underlined letter — Activate that control\nCtrl+C with table focus — Copy the selected shown row\nCtrl+C in Search text — Copy selected search text\n\nPrevious, Next and Clear search are also available as visible buttons.";
pub const TROUBLESHOOTING: &str = "FILE WILL NOT OPEN\nCheck that the ZIP is fully extracted and choose a readable local file of the right format. Keep companion files and fonts beside the app. For a changed file, choose Reload; for another file, choose Open file…. Desktop CSV encoding/delimiter overrides are not available here; use the existing CLI or re-export in your source tool.\n\nNO MATCHES / FEWER ROWS\nRead the checked range and source-row details. Next continues an unfinished search. Clear search removes a filter. Only 64 columns and bounded cell text are shown; a page is not the whole file. Parquet metadata row totals are unverified.\n\nSLOW OR CANCELLED\nChoose Cancel, then Reload or Open file…. Cancel keeps a previous page when available; reading limits may stop before the file ends.\n\nGUIDE MISSING\nExtract the complete portable ZIP, including help/index.html. Open that file in a browser if the system cannot open it for you.\n\nREPORT A PROBLEM\nRecord the app build, Windows version, action, error and shown coverage. Share a small synthetic example, not private data or credentials.";
pub const ABOUT: &str = "Made by Maria\nFree and open source (Apache-2.0).\n\nLocal, read-only CSV and Parquet viewers. No telemetry, uploads or automatic source-file reopening. Desktop browsing, the terminal viewer and Explorer preview have different limits; see the offline guide.\n\nCinzel, Outfit and JetBrains Mono are privately loaded when bundled under the SIL Open Font License. System fonts are used for fallback; no fonts are installed globally.\n\nLicence, notices and third-party licence texts are included in the portable folder. Native Windows accessibility, real monitor changes and signed release packaging remain separate acceptance items.";

#[cfg(test)]
mod tests {
    #[test]
    fn guidance_is_actionable_and_keeps_search_limits_explicit() {
        assert!(super::empty(true).contains("CSV"));
        assert!(super::empty(false).contains("Parquet"));
        assert!(super::no_matches(true).contains("Next"));
        assert!(!super::no_matches(true).contains("complete"));
        assert!(super::no_matches(false).contains("checked range"));
        assert!(super::HELP.contains("first 64 columns"));
        assert!(!super::HELP.contains("64 MiB"));
        assert!(super::HELP.contains("Made by Maria"));
    }
}
