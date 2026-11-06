# Runtime state-machine audit

Scope: CSV Viewer, Parquet Viewer, their desktop workers, Explorer preview handlers,
and the mirrored viewer shell. This is a subsystem inventory, not a claim that a
finite test run proves every scheduler interleaving. UI state is often encoded in
`Option`, generation IDs, counters, HWND ownership and mutex-protected fields,
not an `enum`.

This audit does not change file associations, Low-IL policy, spool isolation policy,
source limits, Explorer wire formats or default applications. Tooling/build scripts
are outside the runtime inventory.

## Inventory

Shared paths below are relative to `crates/viewer-shell`. CSV/Parquet-only paths
refer to the named sibling repository.

### 01. Document completion (shared)

- Transition/invariant: Idle → pending generation → success/error/cancel; consume a completion once.
- Source/coverage: `src/lib.rs; tests/transitions.rs`.

### 02. Worker execution and delivery (shared)

- Transition/invariant: Running → cancelled/delivered; receipt time is not execution time.
- Source/coverage: `src/supervisor.rs; supervisor::tests`.

### 03. Child/watchdog/spool ownership (shared)

- Transition/invariant: Reserve slot → spawn/publish → transfer → reap → drop spool/release slot → deliver; panic must also clean up.
- Source/coverage: `src/supervisor.rs; supervisor::tests`.

### 04. Pending native operation (shared)

- Transition/invariant: The request, generation and task are one owned tuple; failure/cancel drops the whole pending operation.
- Source/coverage: `src/win32.rs; native-smoke.c`.

### 05. Native message/repaint reentrancy (shared)

- Transition/invariant: Mutable event handler → immutable paint snapshot; nested messages cannot obtain a second mutable state borrow.
- Source/coverage: `src/win32.rs; native-smoke.c`.

### 06. Selection, focus, button and clipboard (shared)

- Transition/invariant: No selection/row selection; enabled/disabled, hover/pressed/focus; row replacement resets selection. Copy reads the visible table.
- Source/coverage: `src/buttons.rs, src/win32.rs; tests/button_material.rs; native-smoke.c`.

### 07. Browse/search cursor and source epoch (shared)

- Transition/invariant: Draft query → submitted query → successful page; history only commits on success and never crosses source stamps.
- Source/coverage: `src/browse.rs; tests/browse.rs; tests/transitions.rs`.

### 08. Appearance and window geometry (shared)

- Transition/invariant: Defaults → loaded/fitted normal bounds → normal/maximized/minimized; never restore minimized, persist normal bounds.
- Source/coverage: `src/preferences.rs, src/win32.rs; tests/preferences.rs; native-smoke.c`.

### 09. Preference transaction (shared)

- Transition/invariant: Validate → session-local save lock → temporary file → atomic replacement; errors preserve the old snapshot.
- Source/coverage: `src/preferences.rs; tests/preferences.rs`.

### 10. Help and offline-guide recovery (shared)

- Transition/invariant: Menu/topic/modal dialog/return; guide path resolution → system association or manual-open fallback.
- Source/coverage: `src/guidance.rs, src/win32.rs; tests/help.rs; native-smoke.c`.

### 11. CLI option consumption (both)

- Transition/invariant: Option → value/flag → validation → launch/error; explicit CLI theme remains an override.
- Source/coverage: `CSV crates/csv-desktop/src/lib.rs; Parquet crates/desktop/src/lib.rs; CLI tests`.

### 12. Legacy CSV viewer state (CSV)

- Transition/invariant: No file → loaded preview; parse/reload/options/help/scan commands preserve the last valid result on failure.
- Source/coverage: `crates/csv-viewer/src/app.rs; app tests`.

### 13. Encoding detection and decode buffers (CSV)

- Transition/invariant: Bounded sample → chosen decoder; input/pending output/EOF/final flush; replacement reporting is not hidden.
- Source/coverage: `crates/csv-core/src/encoding.rs; encoding and fixture tests`.

### 14. Quote/escape and record boundaries (CSV)

- Transition/invariant: FieldStart/Unquoted/Quoted/QuoteClosed/Escaped → EOF or bounded halt; quote precedence must agree with the parser.
- Source/coverage: `crates/csv-core/src/scan.rs; quote_guard_agrees_with_csv_parser_when_escape_equals_quote`.

### 15. Scan completeness and bounded halts (CSV)

- Transition/invariant: Header inference → rows/progress → exact EOF or preview/record/byte/time/cancel/parse stop; partial work cannot claim exact totals.
- Source/coverage: `crates/csv-core/src/scan.rs; core scan/fixture/warning tests`.

### 16. Column/date/market/warning accumulators (CSV)

- Transition/invariant: Unobserved → classified/ambiguous; missing values, previous close, date extrema and capped examples update before finalization.
- Source/coverage: `crates/csv-core/src/{types,table,market,warnings}.rs; market/fixture/golden tests`.

### 17. CSV checkpoint browsing (CSV)

- Transition/invariant: Detect source → restore validated checkpoint → bounded read → successful next cursor; changed source invalidates cursors.
- Source/coverage: `crates/csv-core/src/scan.rs; crates/csv-core/tests/browse.rs`.

### 18. CSV RPC framing and worker dispatch (CSV)

- Transition/invariant: Frame length → body → decode; Hello → Ready → Scan/Cancel/Shutdown; wrong versions/truncation fail closed.
- Source/coverage: `crates/csv-protocol; crates/csv-worker/src/main.rs; worker/protocol tests`.

### 19. CSV cancellation request handoff (CSV)

- Transition/invariant: Received/queued ID → begin/active cancellation → end; an old ID must not poison the next request.
- Source/coverage: `crates/csv-worker/src/main.rs; control_tests`.

### 20. CSV spool session (CSV)

- Transition/invariant: Spooling → SpoolReady/Failed/Cancelled; bounded copy publishes progress, owned spool outlives its readers.
- Source/coverage: `crates/csv-preview/src/session.rs; session integration tests`.

### 21. CSV full-scan scheduling/supervision (CSV)

- Transition/invariant: SpoolReady → Scanning → Complete/Failed/TimedOut/Cancelled; spawn errors and owner drop must leave Scanning.
- Source/coverage: `crates/csv-preview/src/session.rs; owned_source_tests; session integration tests`.

### 22. CSV COM stream/apartment and HWND lifecycle (CSV)

- Transition/invariant: Initialize → parent/stream binding → marshal/unmarshal → preview/scan → unload/release; STA waits pump COM calls.
- Source/coverage: `crates/csv-preview/src/windows.rs; COM/marshalling and Windows tests`.

### 23. CSV DLL scheduled-thread ownership (CSV)

- Transition/invariant: Reserve counted closure before OS scheduling → execute/drop → release token; no uncounted queued-thread window.
- Source/coverage: `crates/csv-preview/src/lifecycle.rs; lifecycle::queued_tests; counted spawn sites`.

### 24. Registration transaction and uninstall (both)

- Transition/invariant: Inspect foreign/current ownership → apply/rollback → unregister only owned values; never edit protected UserChoice.
- Source/coverage: `CSV crates/csv-preview/src/registration.rs; Parquet crates/windows registration; registry plan tests`.

### 25. Parquet worker transfer/document protocol (Parquet)

- Transition/invariant: Await Hello → idle → receiving/accepted → inspected/render/sample/scan → shutdown; rejected/consumed streams return idle.
- Source/coverage: `crates/worker/src/lib.rs; crates/worker/tests/protocol.rs`.

### 26. Parquet spool/document capsule (Parquet)

- Transition/invariant: Temporary file → flushed/rewound owned source → metadata/session → document capsule; failed inspection must not retain a consumed spool.
- Source/coverage: `crates/worker/src/lib.rs; crates/worker/src/spool.rs; protocol tests`.

### 27. Parquet sample/scan completeness (Parquet)

- Transition/invariant: Metadata-only → bounded decoded sample/scan → complete/truncated/unavailable/cancelled; exactness requires full decoded coverage.
- Source/coverage: `crates/core/src/{sample,scan,session}.rs; core tests`.

### 28. Parquet worker client/upload/close (Parquet)

- Transition/invariant: Spawn → welcome → declared transfer/chunks/ack → commands → goodbye or kill/reap; short upload aborts, I/O failure returns error and caller drops client.
- Source/coverage: `crates/client/src/lib.rs; client CancelReader tests; protocol/E2E tests`.

### 29. Parquet preview worker/job/COM ownership (Parquet)

- Transition/invariant: Initialize → marshal → attach child to job → transfer/render → cancellation/join/unload; old worker joins before replacement.
- Source/coverage: `crates/windows/src/{handler,stream}.rs; COM tests and preview E2E`.

### 30. Parquet preview parent binding (Parquet)

- Transition/invariant: Unbound → child HWND → repeated same parent/reparent/resize → unload; retain the completion target across SetWindow.
- Source/coverage: `crates/windows/src/handler.rs; rebinding_preview_preserves_window_and_pending_completion_target`.

## Defects corrected

### Terminal document ownership

- Before: An unsolicited/duplicate generation completion could be accepted; Cancel also created a fictitious pending generation.
- Correction: Private pending generation, consume once, explicit cancel transition.

### Queued cancellation and delayed polling

- Before: Queued success could win after Cancel; an already-completed result could be relabelled timed out after a modal dialog delayed polling.
- Correction: Explicit Task phase; cancel dominates undelivered results, receive completed work before checking elapsed UI time.

### Cancellation during worker publication

- Before: UI Cancel waited on the child mutex held during process creation.
- Correction: Nonblocking UI notification to the existing watchdog; fast kill only with try_lock.

### Panic cleanup/capacity

- Before: An adapter panic left a child alive and permanently consumed an operation slot.
- Correction: Catch adapter/transfer unwind, reap before delivery, release the slot through RAII.

### Navigation source epoch

- Before: A committed page with a different source stamp retained prior-source history.
- Correction: Clear history when the committed source stamp changes.

### CSV quote/escape priority

- Before: With escape equal to quote, a successfully parsed CSV was incorrectly marked incomplete/ParseError by the guard.
- Correction: Match the parser's quote-before-escape precedence.

### CSV preview termination/scheduling

- Before: Owner Drop left supervision in Scanning; failed scan/watchdog scheduling returned an error without a terminal transition.
- Correction: Drop uses the cancellation transition; scheduling failure publishes Failed, cancellation and notification without replacing a terminal result.

### CSV scheduled DLL work

- Before: Thread counters were incremented inside thread bodies, leaving scheduled work temporarily invisible to unloading checks.
- Correction: Reserve a counted closure before OS scheduling for spool, scan, timeout and completion-waiter threads.

### Parquet failed/empty transfers

- Before: A rejected empty stream retained a spool; failed inspection retained a spool whose file had been taken.
- Correction: Reject empty declarations before allocation; consume the receiving state before attempting to open the document.

### Parquet repeated parent binding

- Before: SetWindow compared child HWND to parent HWND, destroying the child on every call and invalidating pending completion targets.
- Correction: Compare actual parent and reparent the existing child instead of recreating it; native Wine regression exercised before/after.

## Coverage and remaining acceptance

- New targeted tests cover completion consumption/cancel, source-stamp changes,
  delayed polling, cancellation/child-publication contention, panic cleanup,
  parser quote precedence, scheduled DLL work, preview drop/scheduling failure,
  rejected/consumed Parquet streams and actual native preview reparenting.
- CSV queue registration, begin, cancellation and end share one state lock.
  Cancellation of an active ID is consumed by that scan. A cancellation for
  an already queued scan is consumed once; unknown/completed IDs are ignored.
  Clients must use distinct outstanding IDs and must not send stale cancels
  after reusing an ID, because the wire protocol has no generation token.
- Scan/watchdog scheduling-failure transitions use injected I/O errors in unit
  tests; OS thread exhaustion is not forced on the host.
- Existing workspace tests, explicit CSV worker integration, Linux/Windows GNU
  builds/tests, native smoke, documentation/design checks, packaging/licences and
  hosted MSVC CI provide wider regression coverage. Execution results are recorded
  in the PR and portfolio verification logs, not inferred from this inventory.
- Physical Explorer/default-app chooser, Narrator/UIA, physical-device DPI/high
  contrast and a successful Windows associated-browser handoff remain owner-local
  acceptance. Wine is not a substitute for those checks.
- Process timeouts do not make kernel filesystem calls interruptible. Metadata
  stamps detect ordinary changes; they are not cryptographic content snapshots.
- The CSV request channel is parent-trusted and unbounded; this audit does not
  claim resistance to an independently malicious parent flooding worker commands.
