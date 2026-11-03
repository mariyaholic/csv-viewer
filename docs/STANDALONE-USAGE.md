# Standalone usage

Made by Maria. Free and open source (Apache-2.0).

## Run

1. Extract the whole portable ZIP.
2. Open `csv-desktop.exe`. Keep `csv-worker.exe`, `fonts/`, `help/` and licence files together.
3. Choose **Open file…** or drop one local file into the window.

No terminal, administrator access or COM registration is needed. Source files are read-only.
[The user guide](USER-GUIDE.md) explains every everyday action and recovery path.
The portable folder includes the same guide at `help/index.html`, available from **Help → Offline user guide**.
**F1** opens Quick help; the Help menu also has browsing/search/copy, appearance, keyboard, troubleshooting and About topics.

## Remembered appearance and first use

Theme, normal window bounds and maximized state are remembered on normal close.
Minimized windows reopen normally. CSV and Parquet keep separate appearance-only settings;
no file paths, searches or source data are saved and no file automatically reopens.
See [appearance and recovery](USER-GUIDE.md#remember-theme-and-window-size).

## Browsing and resource limits

The GUI reads bounded pages: up to 200 rows / 64 columns, with 4 GiB CSV and 16 GiB Parquet source caps.
These caps do not guarantee every file below them can be read. No matches in an unfinished slice is not a whole-file absence result.
Parquet metadata totals remain unverified. Copy includes shown, possibly shortened values only.
The desktop `--check` path retains its separate 64 MiB cap.
[Technical paging/search limits](LARGE-FILE-BROWSING.md).

## Build and verify

Developer commands, native smoke arguments, packaging status and source pins moved to [Development](DEVELOPMENT.md).

## Local Windows verification — Maria

The unchecked owner checklist remains in [Development and Windows acceptance](DEVELOPMENT.md).
It covers Explorer/default apps, Narrator/UIA, DPI/monitor/contrast and physical input.
Default-app registration and signed releases are not implemented by this documentation update.
Do not mark native acceptance complete from Wine or CI alone.
