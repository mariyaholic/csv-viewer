# Standalone CSV Viewer — paged desktop browser

Made by Maria. Free and open source (Apache-2.0).

## Run

Extract the whole portable ZIP, then double-click `csv-desktop.exe`. No COM
registration, preview DLL, administrator permission, terminal or installer is
needed. Keep `csv-worker.exe` alongside it. All processing is local/read-only.
Source files are never overwritten. Ordinary launch does not change associations.
Windows 10 1703+ is the provisional API baseline; native Windows acceptance is pending.

Open one local file through Open, Ctrl+O, a single-file drop, or a quoted path
argument. Each launch is independent; use another window for another file.
F5 reloads, Esc cancels, Tab moves focus. Ctrl+F focuses search; Enter/Find applies it.
Previous/Next browse source pages or continue the current search. Select a table row for its shown values
in the inspector; Copy shown row / Ctrl+C with table focus copies only that display.
Control characters are escaped in cells. A visible ellipsis means display truncation,
not the complete original value. Copy does not evaluate formulas or execute links;
pasting untrusted text into other applications remains the recipient's responsibility.

The theme selector changes Paper, Ink, PC-98, Doloris, Sumimi and Marigold on the
loaded document. `--theme doloris` (or another lower-case ID) selects a launch theme.
Theme choice is session-only in this slice. Buttons retain standard native HWNDs
with generated theme faces via NM_CUSTOMDRAW (not BS_OWNERDRAW). Rest/hover/pressed,
focus and disabled state remain distinct; no theme changes the command behavior.
The 40 DIP button hit box includes reserved focus/shadow space around the material.
Native menus, combo boxes and system selection colours remain OS-drawn.
High contrast uses the default native button renderer and overrides authored
colours. Reload is unavailable during loading; Cancel is available only during
an operation; Copy requires a selected row. Access keys are unique.
Paper/Ink use installed UI/mono fonts;
PC-98 requests MS Gothic with Windows fallback. Uika designs privately load Cinzel,
Outfit and JetBrains Mono from `fonts/`; their OFL texts are included. No font is
installed globally and there are no runtime font downloads.

## Browsing and resource limits

The GUI now reads bounded source pages and searches beyond the first 200 rows:
**4 GiB CSV / 16 GiB Parquet source caps**, **200 rows/page**, **64 source columns**,
**4 KiB/shown cell**, **8 MiB display text**, and **128 KiB metadata**. Format
record/decode/deadline limits can stop earlier. There is no full-file editing or
sorting. [Detailed paging/search semantics, format limits and isolation](LARGE-FILE-BROWSING.md).

GUI requests use a read-only, same-executable helper child without copying the
whole source. CSV UTF-8 pages use record checkpoints; other encodings replay the
prefix. Parquet seeks footer/row groups. Search is literal lowercase matching in
the first 64 columns; Parquet searches bounded rendered values, not every raw cell.
A limited search with no matches can still have Next available. Source row counts
from Parquet metadata remain unverified. Copy only includes bounded shown values.

Cancel, replacement, source-change checks, generation rejection and the independent
120-second watchdog remain active. Windows child jobs are kill-on-close; cleanup
and joining do not block the UI. Up to two operations may briefly overlap while
the previous one closes. Source stamps detect ordinary changes, not cryptographic
snapshots. Local regular files are supported; OS-blocked filesystem reads can
outlive cancellation. A failed request retains its prior page; a different file
clears it. No parsing occurs on the HWND thread and no association is changed.

The legacy `--check` inspection path still has its original **64 MiB** input cap,
matched companion worker and private spool. COM/CLI behavior is unchanged. Keep
that worker in the portable bundle; it is not required by the new GUI paging path.
Stale private-spool recovery after a hard crash remains a release follow-up.

## Build and verify

```sh
# Shell/MinGW developer payload with embedded asInvoker / PerMonitorV2 manifest:
bash scripts/build-desktop.sh --debug
# Release-profile path (not a claim that it has been exercised here):
bash scripts/build-desktop.sh
# Pure state, adapters and existing workspace regressions:
cargo test --workspace --locked --offline -- --test-threads=1
# Windows suites (Wine is corroboration, not native accessibility acceptance):
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine cargo test --workspace --target x86_64-pc-windows-gnu --locked --offline -- --test-threads=1
# Legacy inspection adapter (64 MiB cap); diagnostics print to stdout:
csv-desktop --check sample.csv
```

`dist/standalone/` and `dist/csv-desktop-debug-windows.zip` are separate from the
existing COM release payload. GNU builds are unsigned developer builds, not official
MSVC releases. Windows SDK `rc.exe` and the MSVC native-control smoke have been
exercised in Windows CI. Existing CLI/DLL registration commands remain unchanged.

The vendored `crates/viewer-shell` source is version 0.1.0 with exact file hashes
in `crates/viewer-shell/SOURCE.json`. It has no COM dependency or sibling checkout
path dependency. Review/synchronize source explicitly across both repositories.

## Native control smoke host

After a debug portable build, a MinGW developer can run:

```sh
mkdir -p target
x86_64-w64-mingw32-gcc -municode -O2 -Wall -Werror crates/viewer-shell/tests/native-smoke.c -o target/native-smoke.exe -lcomctl32 -lgdi32 -luser32
wine target/native-smoke.exe 'dist\standalone\csv-desktop.exe' 'crates\csv-core\tests\fixtures\golden.csv' CSV target '007'
```

The host verifies real worker values, standard Button identity, unique access keys,
Tab focus, native Space activation, ready/busy/selection availability, BM_CLICK
copy/cancel, six live themes, reload and exit. Add `--browse` after the expected
first-cell argument with a 600-row paging fixture (needle.* at source row 451)
to check live paging and search. `--find CSV "needle.*"` drives the real Find
control for capture/testing. Capture helper modes `--probe`,
`--press`, `--release`, `--hover`, `--away`, `--next`, `--previous`, `--close` take `CSV` as the last argument. Press/release
uses BM_SETSTATE for deterministic native pressed-paint captures, not a claim of
physical pointer automation. Narrator/UIA and actual Windows forced-color/DPI
acceptance are still separate requirements.

![Actual paged/searchable shared-shell controls under Wine](images/native-controls.png)

## Local Windows verification — Maria

**Owner: Maria. Status: pending local verification.** Use an existing Windows 11
Home or Pro desktop; Windows Enterprise, GitHub Enterprise and a paid testing
service are not required. Standard GitHub Actions Windows runners are free for
public repositories, but CI does not replace the desktop checks below.

CI covers the Windows/MSVC build and native-control/paging smoke. The remaining
checks concern the interactive Windows shell, accessibility and display behavior.

### Default-app readiness

The app accepts a quoted file argument. Wine GUI checks passed with spaces and
Unicode in both the executable directory and input filename. This does not prove
Windows Explorer/default-app activation.

**Not implemented yet:** dedicated desktop ProgID/open-command registration,
Open with / Default apps discovery and a register/unregister flow. Existing COM
preview-handler registration is separate. Manually browsing to the EXE through
Open with is the current local test route, not a verified integration feature.
Windows must obtain the user's default-app choice through system UI; do not write
UserChoice or silently replace an existing default.

### Local checklist

- [ ] Extract the entire portable build to a stable folder. Keep `csv-worker.exe`
  beside `csv-desktop.exe`, and keep `fonts/` and licence files. Moving the folder later
  can invalidate a file association.
- [ ] Record the current default for `.csv`. For a small local test file,
  use **Open with → Choose another app**, browse to `csv-desktop.exe`, and select
  **Always** / **Always use this app** if offered (wording varies by Windows).
- [ ] Double-click another `.csv` file after closing the app. Confirm the
  right document/data appears, including paths containing spaces and Unicode.
  Confirm the selection in **Settings → Apps → Default apps**. Restore the prior
  default after testing if desired. A failure here remains an open issue.
- [ ] Check Open dialog, drag/drop, Tab/Shift+Tab, Enter/Space, access keys,
  cancellation and focus return. Check Narrator reads command names, table cells,
  selection, inspector and status updates meaningfully.
- [ ] Check all six themes at 100/150/200% scaling, move between monitors with
  different scaling if available, and enable a Windows contrast theme. Labels,
  focus and selection must remain readable; authored button colors should yield
  to native rendering in forced colors. Check bundled-font loading/fallback.
- [ ] Record Windows edition/build, app build/commit, theme/scaling, pass/fail
  results and screenshots alongside this checklist. Leave untested items open.

The documented GUI paging/search caps still apply when selected as the default;
the legacy `--check` inspection path remains capped at 64 MiB. Do not mark native acceptance complete
from a successful build, Wine test or CI smoke alone.

References: [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
and [Windows app defaults](https://learn.microsoft.com/en-us/windows/apps/develop/windows-integration/default-apps-platform).

## Acceptance still required

Native Windows keyboard/dialog/drop, Narrator/UIA cell and status reading,
100/150/200% DPI and real monitor changes, high contrast, long paths, and font
fallback. Standard controls are not automatically an accessibility pass. Full
S0 gate remains provisional until that native bench run; no toolkit acceptance
is implied by Wine. File override settings, explicit scans, better structured-cell
navigation, performance baselines, persisted theme settings,
crash-spool cleanup and final signed/MSVC packaging are follow-on milestones. Parquet M4
isolation approval is separate and unchanged.
