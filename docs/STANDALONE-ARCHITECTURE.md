# Standalone viewers — architecture proposal

Made by Maria. Status: proposed, not implemented. Prepared 2026-11-02.

## Decision and scope

Build **CSV Viewer** and **Parquet Viewer** as sibling Windows desktop apps:
a portable GUI executable plus its version-matched worker, usable without COM
registration, Explorer integration, an installer, elevation or a terminal.
Retain the current preview DLLs and command-line tools as independent entry points.
This realizes CSV's optional companion-window intent and explicitly extends
Parquet's original preview-only scope at the owner's request. It does not assert
that a GUI already exists or approve changes to Explorer isolation.

Reader: owner and implementer. Decision requested: accept this Windows-first native
shell and milestone sequence. The first delivery is a usable bounded inspector,
not a spreadsheet, SQL workbench or arbitrary full-file browser.

### Observed starting point

- CSV has an interactive terminal companion, not a desktop GUI:
  `crates/csv-viewer/src/{main,app,ui}.rs`. Its full-scan thread calls
  `csvview_core::scan_file` in-process. Do not use that as the GUI isolation model.
- CSV `crates/csv-core/src/table.rs` virtualizes **retained preview rows**;
  it is not an on-disk random-access row index. `ScanRequest` has preview row
  counts, not a page-offset API. Full scan does not retain every source row.
- CSV `crates/csv-preview/src/backend.rs` already owns worker startup, protocol,
  cancellation, deadlines and a private-spool contract. Session and quick-head
  parsing in `session.rs` are coupled to the preview lifecycle; do not blindly
  import the DLL as the desktop application's controller.
- Parquet `crates/cli/src/main.rs` is a headless diagnostic tool. It directly uses
  `Session::open_path`, not the isolated-worker client.
- Parquet `crates/client/src/lib.rs` exposes `WorkerClient::{open_stream,inspect,
  sample,scan,diagnose}` and version validation. Calls are blocking; its module
  explicitly documents **no wall-clock deadline**. A UI thread must not call it.
- Parquet has bounded sample offsets (`Limits::max_sample_offset` defaults to
  1,000,000), not unlimited browsing. The current `open_stream` transfers the
  declared file into a worker spool before inspection; large-file startup is not
  a footer-only seek. Do not market instant multi-gigabyte opening on that path.

## Product contract

Double-click the app → an empty window with **Open file…** and a visible explanation
of read-only/local operation. A path argument or file drop opens the same flow.
A ZIP containing GUI + worker works on a machine with no registered handler.
Missing/mismatched workers produce an actionable in-window error, not a silent exit.

- One document per window in the first release; multiple windows are allowed.
  No global single-instance mutex that discards a second file-open request.
- Open dialog, drag/drop, quoted Unicode path argument, recent folder (not recent
  file contents), Reload, Cancel, keyboard navigation, Copy, Settings and About.
  Dropping multiple files opens separate windows after a bounded confirmation;
  never silently inspect only the first file.
- CSV: encoding/delimiter/header overrides, retained sample table, review notes,
  scan coverage, explicit Full scan, optional existing date/market settings.
- Parquet: schema, row-group metadata, bounded sample pages, structured nested-cell
  details, scan coverage and optional existing diagnostics. Footer row count is
  always labelled **metadata-reported**, not independently verified.
- No editing, saving back, sorting the whole dataset, global search, SQL, conversion,
  live market feeds, uploads or telemetry. Find in loaded rows may ship only with
  that scope stated in its label. Copy is an explicit clipboard write, not source
  modification; show truncated-value indicators and never evaluate formulas or links.

## Architecture: common presentation, separate format engines

```text
Windows launch / dialog / drop
              |
              v
  Desktop shell [UI thread]
  commands -> pure DocumentController -> immutable ViewModel -> view
                       |                         ^
                 bounded task queue              | bounded events
                       v                         |
              Format adapter / supervisor [I/O threads]
                       |
             versioned bounded worker protocol
                       |
                existing format worker
                       |
              existing format core parser

Explorer -> existing COM DLL -> existing worker path [unchanged]
Terminal / diagnostic CLI -> existing path [unchanged initially]
```

There must be **no dependency from the desktop shell to the COM DLL** and no
registry writes during ordinary launch. Format engines never import HWND, theme
or widgets. Shell code never parses CSV/Parquet or guesses coverage.

### Ownership and proposed modules

Each workspace gains a format-specific desktop package and pure app-model package:
CSV `crates/csv-desktop`, `crates/csv-app`; Parquet `crates/desktop`, `crates/app`.
These names are proposals, not existing directories.

- `app`: commands, document generation, state reducer, capabilities, selection,
  viewport, coverage and error models. Cross-platform tests, no Win32 dependency.
- `adapter`: maps existing format reports to bounded view data without erasing
  format semantics. CSV text stays text; Parquet distinguishes null, empty string,
  binary, nested and display-truncated values. Preserve stable column IDs where
  headers repeat. Formatting is never used as the value's identity.
- `supervisor`: child lifecycle, read-only source ownership, transfer, handshake,
  deadlines, cancellation, cleanup and bounded event delivery.
- `win32`: message loop, window/dialog/drop input, menus, controls, DPI, clipboard
  and accessibility. Handles belong to the UI thread; RAII owns OS resources.
- `view`: layout shared by paint, hit testing and accessibility bounds; it does not
  own document state. A screenshot seam renders this same view, not a separate mock.
- `theme`: semantic tokens, palette registry and font fallback; no parser knowledge.

Define the reusable presentation source as a small `viewer-shell` package with
its own version and tests. **Do not create a new Git repository or published crate
just for milestone one.** Prove it first in CSV, then vendor the exact reviewed
source into Parquet with version, source commit and content hashes recorded in a
manifest. CI checks the pinned file inventory; synchronizing a change is an
explicit two-repo update. No absolute path dependency, sibling-checkout dependency,
submodule or network fetch during a release build. Extract to an owner-approved
shared repository only when both apps demonstrate stable common boundaries.
Format adapters and workers are never vendored into that shared package.

### Native UI decision

Recommend Rust + Win32 with standard menus/dialogs, native keyboard/focus handling,
and an owner-data table control for the first spike. This fits both Rust engines
and the lightweight native portfolio. Start with standard system chrome, not a
frameless custom window. Use the workspace-compatible Windows bindings after
MSRV, licence and transitive-dependency review; do not introduce a second bindings
family into the same workspace without a reason.

A browser shell (Electron/Tauri/WebView2) would simplify CSS themes but introduces
a runtime/rendering stack and a new accessibility/security integration surface.
An immediate-mode GPU UI offers fast custom tables but changes baseline hardware,
packaging and accessibility requirements. Neither is justified before a native
spike. Win32 is not automatically cheap: keyboard selection, cell-level Narrator
exposure and dark table rendering must be proved, not assumed.

**Gate:** a real owner-data table must pass keyboard and Narrator/UIA cell reading,
100/150/200% DPI, high contrast, horizontal scrolling and the supported column
limit. If it cannot, stop and record a toolkit decision rather than quietly ship
an inaccessible custom-painted grid. Theme scope may narrow before accessibility.
Do not build an entire custom grid/UIA provider just to reproduce ticker colours.

## Lifecycle, cancellation and trust

`Empty -> Opening -> PreviewReady -> Scanning -> PreviewReady` with explicit
`Cancelling`, `Failed` and `Closed` transitions. Keep a loaded preview visible during
scan and scan failure. Every open/reload/override increments a document generation;
every page/scan has a request ID. Ignore late events with a stale pair. On open of
file B, no result or error from A may alter B's window.

The supervisor must retain an independent termination handle while a reader blocks
on worker I/O. Setting an atomic flag alone cannot interrupt `Read`, handshake,
write or decode. Cancellation kills the child when graceful shutdown cannot finish,
closes pipes, joins task threads off the UI thread and removes private spool data.
Use a Windows job with kill-on-close for owned children where supported, but do not
call a job object a security sandbox. Record disposal failures without file contents.

Bound queued pages and progress events; coalesce progress, never lose terminal
success/error/cancel events. One active operation per document initially; changing
CSV overrides cancels and reopens. Scroll requests are latest-wins. Preserve the
worker's version handshake, frame bounds and stricter existing parser limits.

Proposed operation defaults: 10 s startup/handshake, 120 s scan, and a separately
visible 120 s initial local transfer deadline, subject to validation on actual
supported storage. No invented percentage while waiting on an unmeasurable task;
show phase and bytes/records when known. A timeout is a partial/unavailable result,
not successful completion. Do not silently raise existing worker limits.

Open source handles read-only, without exclusive write locks; reject directories
and devices, handle access denial and long/Unicode paths. Reopening or full scan
must not silently mix different source versions. Hold a stable handle/snapshot;
if reopening is unavoidable compare file identity/length/write time and invalidate
results on change (those checks alone are not a cryptographic immutability guarantee).
No shell execution of paths, URLs or formula-like cells. Clipboard output is bounded;
copying truncated data must be explicit, not presented as the complete original.

### Input and paging adaptations

**CSV first delivery:** worker-backed bounded sample plus explicit full scan.
Extract the existing non-COM worker backend into a reusable client package, leaving
COM lifecycle accounting in the DLL; preserve existing DLL tests. Add a bounded
preview operation/protocol revision if needed so the GUI does not parse untrusted
quick-head data in-process. Keep old and new worker compatibility explicit.
The first GUI scrolls retained sample rows only and says so. Arbitrary CSV paging
requires a later cancellable, encoding/dialect-aware record index respecting quoted
newlines, snapshot identity and index/disk limits; it is not a `seek(line)` shortcut.

**Parquet first delivery:** reuse `WorkerClient` behind the supervisor, with an
independently interruptible watchdog. Stream/spool opening is acceptable only with
visible transfer progress and documented disk cost. Sample pages stay within core
limits; cache a bounded LRU (proposed 4 pages, 200 rows per page, 8 MiB total rendered
text) and cancel obsolete work. Enforce both row and byte budgets; huge values and
schemas cannot bypass the memory cap. A full scan must not evict/replace the sample
with all decoded rows.

A subsequent standalone-only local-file transport may avoid the full spool by
passing an already-open read-only handle to the child, then using random-access
reads. It requires a new protocol/handle-lifetime design and Windows security tests;
never send arbitrary paths to the worker as an unreviewed fast path. COM remains
stream-based. Neither desktop transport is a fix or workaround for Parquet M4's
unresolved native Explorer/Low-IL spool decision. Do not change that isolation gate.

## Visual direction: a quiet inspection instrument

Audience: people opening unfamiliar exports and datasets, not running a trading desk.
Concept: a compact field instrument that keeps evidence and uncertainty beside data.
Three words: **inked, precise, restrained**.
References: MH Ticker's Paper and PC-98 panels; local-trading-desk's task-specific
native tabs; mic-mute's text-backed status and recoverable window lifecycle.
Density: dense instrument, with a calm table plane and one contextual inspector.
Forbidden: dashboard card walls, trading charts, ticker docking, fake totals,
scanlines over cell glyphs, animated data backgrounds and colour-only status.

Direction inherits the estate's semantic colour roles (`design/uika-canonical.css`),
not an arbitrary new palette. MH Ticker informs the choice mechanism and alternate
skins. Keep gold for focus/actions, lavender for information, explicit warning/error
and success roles. All proposed colours require measured contrast on real controls;
existing ticker dim labels are not evidence of accessibility compliance.

```text
CSV Viewer — export.csv                         Open  Reload  Theme  Help
UTF-8 · comma · header present          [Overrides]   Read-only
[Table] [Review notes] [Scan details]                 [Full scan] [Cancel]
+--------------------------------------------------+-------------------+
| # | account | timestamp | amount | ...            | Selected cell     |
|   | unchanged source values / keyboard selection | value / type /    |
|   | native scrollbars; frozen header + row labels | truncation detail |
+--------------------------------------------------+-------------------+
Partial · 200 sampled rows · coverage/reason supplied by the engine
```

Parquet uses Table / Schema / Row groups / Scan details; the inspector explains
nested values and metadata provenance. Below the minimum comfortable width, collapse
the inspector into a labelled drawer, never shrink text until it fits.

Theme contract: background, surface, primary/secondary text, grid, selection,
focus, accent, information, warning, error, success; shared spacing, type, radius
and motion roles. One data catalogue drives both menu and rendering, as in ticker.
Ship **System**, **Paper** and **Ink** first; add **PC-98** only after contrast and
DPI acceptance. System follows Windows appearance and high contrast overrides all
skins. Unknown theme IDs fall back safely. Gold-accent retro chrome may be square
and pixel-influenced, but data uses scalable readable mono with glyph fallback.

Use installed fonts without redistributing them; conventional native control text
is deliberate, not the product identity. Data defaults to Consolas with system mono
fallback; optional font choice must not change value semantics. Brand typography
can follow estate Cinzel/Outfit/JetBrains Mono only after embedding/redistribution
rights and glyph coverage are checked. No font downloads at runtime.

Logical units: 4/8/12/16/24 spacing, 4 control radius, 16 body text and a comfortable
row height derived from font metrics (proposed 28 at 96 DPI). Compact density is an
explicit setting. Flat surfaces, no shadows behind every row. Recompute layout and
fonts on DPI/theme changes; preserve focus and selection. Respect reduced motion;
static themes have no repaint timer, and hidden/minimized windows do no animation.
Focus/selection and status retain shape/text distinctions in every theme.

## Delivery and acceptance gates

1. **S0 — native shell spike.** Separate GUI binary opens without a console or COM
   registration; dialog/drop/Unicode launch, owner-data table, keyboard/UIA, DPI and
   high contrast proved with a fixture. Reject or narrow the toolkit at this gate.
2. **S1 — CSV vertical slice.** Empty/Open/Error/Table/Review/About flow, worker-backed
   sample, truthful coverage, overrides, cancellation and missing-worker recovery.
   Independently test reducer and stale-event rejection; compare values with fixtures.
3. **S2 — Parquet sibling.** Pin the reviewed shared shell, adapt worker reports,
   schema/row groups, bounded sample pages and nested-cell inspector. Exercise transfer
   progress/disk-full, read failures and cancellation of blocking I/O.
4. **S3 — explicit scans and themes.** Scan/cancel/reload, CSV date settings and existing
   optional diagnostics; System/Paper/Ink and accessibility-tested PC-98. No new market
   analysis features. Screenshot tests go through the real view and layout.
5. **S4 — portable release.** ZIP includes GUI + exact worker + notices + checksums;
   existing CLI/DLL artefacts remain available. Build GUI with Windows subsystem and
   PerMonitorV2 manifest. Test no DLL present, no registration and no admin rights.
   Keep preview installation and file-association changes separate opt-in actions.
   Only an installer changes Open With/default associations, with explicit consent
   and safe uninstall; portable launch never changes them. Signing remains an owner gate.

Tests at each relevant gate: malformed/empty/huge inputs; quoted multiline CSV,
leading zeros, duplicate headers, Unicode and encodings; Parquet null/binary/nested
values, corrupt footer and metadata-versus-scan counts; worker mismatch/crash/hang;
rapid A→B→A open, repeated Cancel/Reload/close, bounded queues/cache/spool; source
change; disk-full; paths with spaces/non-Latin characters; visible copy truncation.

Keep Linux pure-model/parser tests, existing Windows/Wine worker and COM tests,
and release hash checks. Add real Windows GUI acceptance: keyboard-only Open/Copy/
Cancel/Close, Narrator cells and status announcements, 100/150/200% DPI and monitor
changes, high contrast, restored off-screen bounds, no console flashes or registry
writes. Wine corroborates lifecycle and integration; it does not certify Narrator,
Explorer, Windows theme behaviour or physical display acceptance.

Performance measurements, not promises: capture startup-to-empty, open-to-first
sample split into transfer/decode/render, UI input latency under load, idle CPU,
private bytes, peak worker memory, spool bytes and GDI handle growth after repeated
open/close. Provisional targets: paint empty shell within 500 ms on the declared
reference machine, <100 ms UI command acknowledgement during work, no periodic
static-theme repaints, no unbounded growth across 100 opens. Establish baselines
before enforcing budgets; MH Ticker's small resident set is not an Arrow decoder budget.

## Reference record and remaining decisions

Inspected local source heads: CSV `3ece39a`; Parquet `5627c12`; trading desk
`61d7355`; mic-mute `52b1130`. Paths above are relative to those repositories.
MH Ticker reference commit: `27e74bcca022e5da2156a6142c3faf474eb444b9`.
Read `README.md`, `ARCHITECTURE.md`, `THEMES.md`, `src/theme.rs`; visually inspected
`shots/paper.png` and `shots/pc98.png`. Sources:
https://github.com/mariyaholic/mh-ticker/tree/27e74bcca022e5da2156a6142c3faf474eb444b9

Borrow ticker's token catalogue, semantic status separation, cached resources and
real-painter screenshot seam. Do **not** reproduce the growing monolithic `ui.rs`
that its own architecture flags, or copy its network, alerts or docking subsystems.
Other portfolio sources: trading desk `internal/ui/ui_windows.go` (tabs, DPI/font
rebuild), mic-mute `internal/platform/windows/ui_windows.go` (coalesced UI update
and lifecycle). These are architectural references, not verified reusable Rust code.

Owner decisions remaining: approve the proposed Windows-first scope/toolkit after S0;
confirm whether sample-only CSV browsing is enough for the first GUI; select supported
Windows baseline and reference test machine before packaging. Cross-platform GUI,
whole-file CSV paging and spool-free Parquet transport are follow-ons, not hidden
requirements. No code reuse or asset redistribution from a reference bypasses licence
review. No GUI implementation, benchmark or native accessibility pass is claimed here.
