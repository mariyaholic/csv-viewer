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
pub const HELP: &str = "Open file… chooses a file. You can also drop one file into this window. Files are read-only; the viewer does not change them.\n\nPrevious and Next move between pages. Search text + Find searches from the beginning, ignoring letter case. Search is literal, not a pattern. Only the first 64 columns are searched. The status and file details report the checked range; Next continues an unfinished search. Clear search returns to the first unfiltered page.\n\nSelect a row to see its values on the right. Copy shown row copies the displayed values only; … means shortened text.\n\nReload rereads a changed file from the beginning. Cancel stops reading and keeps the previous page, when available. Open file… lets you choose another file after an error.\n\nChoose a Theme, move or resize the window, then close normally to remember the appearance. CSV and Parquet keep separate preferences. A minimized window reopens normally, not minimized. No file history or searches are saved.\n\nKeyboard: Ctrl+O open · Ctrl+F search · Enter find · F5 reload · Esc cancel · F1 help · Tab move between controls.\n\nMade by Maria\nFree and open source (Apache-2.0).\nPrivate OFL fonts: Cinzel, Outfit, JetBrains Mono when supplied.";
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
