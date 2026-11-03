# CSV Viewer user guide

Made by Maria. Free and open source (Apache-2.0).

## Start here

Use this desktop app to inspect a local CSV file without changing it.
You do not need a terminal, administrator access or Explorer preview registration.
This is a read-only viewer, not a spreadsheet editor.

1. Extract the entire portable ZIP to a folder you can keep.
2. Open `csv-desktop.exe` inside the extracted folder.
3. Choose **Open file…**, select a local file, then choose Open in the file picker.
4. Select a row to see its displayed values on the right.

Keep `csv-worker.exe`, the `fonts` folder, the `help` folder and licence files with the app.
Do not run only the EXE from inside the ZIP.
Developer builds are unsigned. Windows may warn about an unsigned download; verify its source and follow your device or organisation's security policy. Do not disable protection to run an unknown copy.

## Open a file

Choose **Open file…**, press Ctrl+O, or drop one file into the window.
Open another app window to inspect another file. Opening a different file clears the previous search.
Opening and browsing do not write to your source file.

The filename in the heading and window title identifies the current file.
If a file is still being written by another program, wait for that program to finish, then use **Reload**.
For a stable comparison, export a separate local copy from that program first.

## Browse rows and inspect values

**Next** reads the next page. **Previous** revisits a successful page you already saw.
A page contains up to **200 rows** and the first **64 columns**. Text limits can make a page smaller.
The table is not a view of every row loaded into memory.

Select a row for the values shown in the right-hand inspector. Read the source-row label and checked range before interpreting a result.
CSV source rows count data records after the detected header, not physical lines; a quoted multiline record is one row.
Parquet row totals reported by metadata are unverified, not an independent count of decoded rows.

## Find text

1. Click **Search text** or press Ctrl+F.
2. Type the text to find, then choose **Find** or press Enter in that field.
3. Read the checked range in the status and file details.
4. If **Next** is available, choose it to continue the search.
5. Choose **Clear search** to return to the first unfiltered page.

Find starts at the beginning and ignores letter case. It matches literal text: `needle.*` means those characters, not a regular expression.
Only the first 64 columns are searched. This is not SQL, sorting or a full-file index.
A no-match page with Next available does not mean the entire file has no matches.
An exhausted checked range does not prove absence from excluded columns or unrendered values.

CSV searches decoded field text within its parser limits, including text longer than the shorter display limit.
Parquet searches bounded rendered values, not unrendered nested/binary content or shortened string tails.
F5 reloads from the beginning with the applied search. Cancel stops an active read; it does not clear the filter.

## Copy a row safely

1. Select a row in the table.
2. Choose **Copy shown row**, or press Ctrl+C while the table has focus.
3. Paste into the application you intend to use.

Copy includes displayed values only. An ellipsis (`…`) means text was shortened; it is not the complete original value.
Control characters are escaped. Use the source application's export tools when you need complete original values.
Ctrl+C inside Search text copies selected search text instead of a table row.
The viewer does not execute formulas or links. A spreadsheet or another recipient may interpret pasted untrusted text differently.

## Remember theme and window size

Choose **Theme**, move or resize the window, then use **normal close** to save appearance.
The six choices are Paper, Ink, PC-98, Doloris, Sumimi and Marigold. They do not change the data or search behavior.
Each viewer saves its own theme, normal size/position and maximized state. A minimized window reopens normally.
Saved bounds are fitted to the current monitor work area. With several windows, the last successfully saved appearance wins.
Crashes, forced termination and a failed save do not save your latest changes.

Settings contain appearance only: no source file paths, searches, data or clipboard contents.
The app does not automatically reopen your last file. Windows may maintain its own file-picker navigation state separately.
The settings location is `%LOCALAPPDATA%\Maria\Viewers\csv-viewer\desktop.json`.
Moving the portable folder does not move those per-user preferences.

If saved appearance cannot be read, the app uses defaults and preserves the existing settings.
Press **F1** for a reset prompt. **No** keeps the old settings; **Yes** resets only appearance and enables a fresh save on normal close.
Do not delete or edit your source file to repair appearance.

## Help without an internet connection

Choose **Help** for Quick help, Offline user guide, Browsing/search/copying, Appearance/settings, Keyboard shortcuts, Troubleshooting or About.
F1 opens the compact quick reference directly. About contains the author and licence information.
The offline user guide opens `help/index.html` beside the EXE in your default browser. It has no scripts, external fonts or network resources.
If no browser is configured, open that file manually in an installed browser. The guide does not send your source file to the browser.
Use the browser's Find or Print commands to search or print the guide.

## Keyboard shortcuts

- Ctrl+O: Open file.
- Ctrl+F: Focus and select Search text.
- Enter in Search text: Find from the beginning.
- F5: Reload with the applied search.
- Esc: Cancel an active read.
- F1: Quick help.
- Tab and Shift+Tab: Move between controls.
- Space on a focused button: Activate it.
- Alt plus an underlined letter: Use that control's access key.
- Ctrl+C with table focus: Copy the selected shown row.
- Ctrl+C in Search text: Copy the selected search text.

## Fix a problem

### The file will not open

Check that it is a readable local regular file of the right format and that extraction is complete.
Use **Open file…** to choose another file. Use **Reload** after an ordinary source-file change.
A different filename extension does not convert a file's format.
CSV encoding, delimiter and header inference can be wrong for unusual input; the desktop app has no override controls yet. Re-export from your data tool or use the existing CSV terminal tools.

### No matches or fewer rows than expected

Check for an active search, the source-row label and the checked range.
Use **Next** to continue an unfinished search, or **Clear search** to browse normally.
Only 64 columns and bounded values are shown or searched. A page or a Parquet metadata count is not full-file validation.

### Reading is slow, cancelled or stopped by a limit

Choose **Cancel**. A previous page is kept when available; the status tells you whether reading stopped.
Choose **Reload** to start again with the current filter, or **Open file…** for another file.
Limits protect the app, and can stop reading before the file ends. Cancellation cannot guarantee immediate return from an OS-blocked filesystem read.
Later UTF-16 or Windows-1252 CSV pages may hit the time limit while replaying earlier records.

### Appearance is not remembered

Close normally and reopen. Check whether another window closed later or an explicit launch theme replaced the saved choice.
Read any appearance warning. If settings cannot be read, press F1 and choose whether to reset them.
If the local settings folder is unavailable or unwritable, viewing still works but appearance cannot be saved there. The app does not write preferences beside your data as a fallback.

### The offline guide is missing

Extract the entire portable ZIP again, including `help/index.html`, into a complete folder.
Help uses the EXE's folder, not the working directory or source-file folder.
Open the HTML file directly if Windows has no suitable default browser.

## Desktop, terminal and Explorer are different

This guide describes `csv-desktop.exe` and its paged desktop interface.
The existing terminal viewer, desktop `--check` diagnostic mode and Explorer preview have separate commands and limits.
Do not apply terminal full-scan switches or CSV dialect overrides to the desktop GUI.
The desktop `--check` path retains its **64 MiB** input cap and matched companion worker. It is not the same as GUI browsing.
Explorer preview registration is opt-in and is not default-app registration.
Dedicated desktop Open with / Default apps registration is not implemented. Ordinary launching does not change your associations or defaults.

## Limits and privacy

- GUI source caps are **4 GiB** for CSV and **16 GiB** for Parquet. These are limits, not a promise that every file below them can be decoded or was stress-tested.
- Each page shows up to 200 rows and 64 columns. Shown cells are limited to 4 KiB, display text to 8 MiB and metadata to 128 KiB.
- Format, record, decode and time limits can stop a read earlier. A partial result is not a complete validation.
- The GUI opens sources read-only in a helper process. It does not upload files, make network requests for data, install fonts globally or send telemetry.
- The helper has your normal read permissions; it is not a new low-integrity sandbox.
- There is no full-file editing or sorting. Displayed or copied values may be shortened.

Native Windows accessibility, real multi-monitor/DPI behavior, high contrast, default-app interaction and signed release packaging still require separate acceptance. Standard controls, Wine tests or CI do not certify those behaviors.

## Report a problem without sharing private data

Record the build or commit, Windows version, action taken, complete error message and shown coverage.
Say whether the problem occurs in the desktop app, terminal viewer or Explorer preview.
Use a small synthetic example when possible. Do not attach personal source data, credentials or private clipboard contents.
The repository's documentation index links developer testing notes and the remaining owner checklist.
