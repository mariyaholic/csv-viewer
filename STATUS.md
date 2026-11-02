# Status

## Standalone desktop — implemented first slice (2026-11-02)

This supersedes the architecture-only note below; it is not native release acceptance.

- Native portable GUI plus companion worker; no COM registration, elevation or
  terminal needed. Open/path/drop, reload/cancel, bounded owner-data sample table,
  shown-row inspector/copy, in-window errors and six live theme choices.
- Paper, Ink, PC-98, Doloris, Sumimi and derived Marigold. Uika OFL fonts are
  process-private; standard controls retain system selection/button rendering.
- Worker supervision includes a modal-independent deadline, cancellation,
  kill-on-close Windows job, bounded transfer/response/display and private spools.
  Reentrant paint reads a separate copied snapshot, not a mutable document borrow.
- Shared shell source is byte-identical and content-pinned in SOURCE.json.
  Existing worker wire protocols, CLI and Explorer/Low-IL behavior are unchanged.
- Debug/unsigned Windows GNU portable ZIP built, extracted payload hashes checked,
  actual app/fixture readback exercised under Wine. Both native smoke hosts verify
  owner-data cells, selected-row inspection, clipboard, reload, six live themes,
  close/reopen. Missing worker errors and read-only source hashes checked.
- Native Windows dialog/drop, keyboard/Narrator/UIA, DPI/high contrast, font
  fallback, clean-machine/MSVC/signing and owner acceptance remain OPEN. Wine
  is corroborating evidence only. No release or new remote PR is claimed.
- See docs/STANDALONE-USAGE.md for exact preview limits and follow-on milestones.

CSV: 145 Linux / 156 Windows tests passed; one ignored integration was also
explicitly enabled and passed on each target.
Strict desktop/shell Windows Clippy and formatting pass. Final shared process
tests were also rerun after fixture-isolation hardening. Full-suite logs and
actual GUI captures live in the portfolio verification directory.

## Standalone desktop architecture — proposed 2026-11-02

- Added `docs/STANDALONE-ARCHITECTURE.md`: GUI boundaries, truthful sample/scan
  capabilities, worker lifecycle, theme contract, staged delivery and acceptance.
- Grounded in both engines, trading desk/mic-mute native UI and pinned MH Ticker
  source plus Paper/PC-98 screenshots. No runtime dependencies or application code changed.
- Documentation/source-reference checks passed; no GUI, benchmark, native accessibility
  validation or new application test pass is claimed. Start implementation at S0.

## Latest hosted verification — 2026-11-02

[measured] Current tracked RS source: 12,955 lines across 34 files,
including tests (raw physical lines, not a production-only LOC estimate).

This section supersedes historical local/CI assumptions below. Changes are on
`fix/remaining-plan-issues`; no push, CI trigger or release publication occurred.

- **07A-15 fixed:** CLI registration and unregistration resolve the existing
  companion `csv_preview.dll`, not the EXE. Missing/present DLL regression
  passes. Actual debug and release Wine CLI registry readbacks point to the DLL;
  uninstall removes the owned tree/leaves and also cleans a legacy EXE-valued
  registration. Release DLL regsvr32 registration/unregistration still works.
- **07A-16 fixed at the handler layer:** early stream/start failures retain a
  child pane with a read-only diagnostic rather than returning before child
  creation. The real DoPreview/reopen regression is red against the original
  implementation and green after restoration. Class registration can recur
  after its last-object unregistration. Native Explorer reproduction remains open.
- **COM test deadlock fixed:** source STA pumps while the background proxy
  reads; the complete-payload assertion remains, with a 20 s failure deadline.
  No marshalling test is skipped.
- **Release notices:** generated offline Cargo licence texts are committed,
  checked by the build scripts, copied/hash-listed with LICENSE/NOTICE and
  THIRD_PARTY, and attached by the tag workflow. Shell/PowerShell script line
  endings are pinned to LF. Python 3 is required at build time for the licence check.
- **Strict Linux lint:** Windows confirmation helper is compiled only on
  Windows or in tests; all portable confirmation regressions remain enabled.

Verified with Rust/Cargo 1.99.1, MinGW GCC 12, Wine 11, serial test execution:
- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`:
  pass on Linux; same command with `--target x86_64-pc-windows-gnu`: pass.
- `cargo build --workspace --locked --offline` and `cargo test --workspace
  --locked --offline -- --test-threads=1`: **134 passed, 1 ignored** on Linux.
- Build/test with `--target x86_64-pc-windows-gnu`, runner `wine`:
  **145 passed, 1 ignored**, unfiltered; both new handler/resolver regressions
  and the formerly hanging cross-apartment case pass.
- The ignored `worker_backend` integration was explicitly run and passed on
  both targets with CSV_VIEWER_WORKER set to the built worker.
- `cargo deny --offline check licenses bans`: pass (allowance warnings).
- `python3 scripts/generate-third-party-licenses.py --check`: pass.
- `bash scripts/build-windows.sh`: real GNU release build passed; SHA256SUMS
  verifies all 7 payload files. Release CLI renders quoted_multiline.csv in Wine.

Evidence: portfolio `verification/FINAL-RESULTS.json`, round-2 logs, CSV
red-regression/registry/release-smoke logs and `release-payloads.json`.
Native Explorer isolation, visual/large-file/DPI/unload checks and native
PowerShell/MSVC packaging remain unverified; `docs/TESTING.md` records each
XR-2 item. CI's pinned Rust 1.85.0 was not installed/run here. Parquet and GNU
runtime licence warnings are not waived by a local cross-build.


Made by Maria. Apache-2.0.

The CSV viewer, parser, bounded preview session, worker, protocol and Windows
Explorer handler are built. Unregistration deletes the owned trees outright —
the CLSID tree (with the Low-IL opt-out inside it) plus the `.csv\ShellEx`
and `SystemFileAssociations\.csv\ShellEx\{8895…}` leaves under HKCU — while
shared keys always stay. Rollback restores prior values when its ownership
check succeeds and does not delete keys.

[measured] This branch implements the Opus-review fix plan for this repository
(14 findings, one commit each, highest severity first): both highs (07A-01
Low-IL opt-out, 07A-02 stream marshalling), all five mediums (07A-03 console
flash, 07A-04 per-field memory bound, 07A-05 panic containment, 07A-06
thread/class lifetimes, 07A-07 visible registration warning plus HKCR
preflight) and all seven lows (07A-08 uninstall deletion, 07A-09 spool-cap
boundary, 07A-10 per-monitor-DPI font, 07A-11 VK codes + click-to-focus +
IObjectWithSite/frame forwarding, 07A-12 honest worker skip + pipeline tests,
07A-13 strict clippy/cargo-deny/Linux job/release job, 07A-14 C2 Notice rule).

## Commits (one per finding)

| Finding | Commit | Summary |
|---|---|---|
| 07A-01 | `987fb90` | opt out of Low-IL process isolation; typed registry values (`REG_DWORD`) |
| 07A-02 | `8b4b7ae` | stream the host input to the session off the host apartment |
| 07A-03 | `2f5ddd5` | suppress the console window on every worker spawn (`CREATE_NO_WINDOW`) |
| 07A-04 | `ee456c6` | bound memory, cancellation and the deadline inside a single record |
| 07A-05 | `7db3927` | contain panics at every COM and Win32 entry point |
| 07A-06 | `e8e7818` | pin the DLL while its threads run and own the window class |
| 07A-07 | `cb037bf` | visible registration warning CLI; conflict preflight reads HKCR too |
| 07A-08 | `4ec6d94` | uninstall deletes the owned CLSID tree and 8895 leaves (C1) |
| 07A-09 | `083913f` | an exactly cap-sized source spools complete, not truncated |
| 07A-10 | `746f290` | child panes handle `WM_DPICHANGED_AFTERPARENT` and rescale the font |
| 07A-11 | `a25aa32`, `ed25177` | hotkey compares VK codes; click focuses the pane; `IObjectWithSite` + frame key forwarding |
| 07A-12 | `d4684d6` | honest worker skip, tests on the real head/spool pipeline |
| 07A-13 | `73659bb` | strict clippy with triaged allows; cargo-deny, Linux job, tag release job |
| 07A-14 | `c86d6ce` | C2 Notice rule in ARCHITECTURE; MinGW runtime notices in THIRD_PARTY |

## Build and test

Commands run locally (Rust 1.98.1; the host toolchain is
`x86_64-pc-windows-gnu`, so the `#[cfg(windows)]` handler code and the COM
tests run for real; `core.autocrlf=true`):

```text
cargo fmt --check                                     # exit 0
cargo clippy --locked --all-targets -- -D warnings     # exit 0 (07A-13 triage)
cargo build --locked                                  # ok
cargo build --locked --target x86_64-pc-windows-gnu   # ok
cargo test --locked                                   # 143 passed, 0 failed, 1 ignored
```

Baseline on `main` was 119 passing; 07A-01 added one test (120), 07A-02 two
(122), 07A-03 one (123), 07A-04 two (125), 07A-05 one (126), 07A-06 three
(129), 07A-07 six (135), 07A-08 two (137), 07A-09 two (139), 07A-10 two
(141), 07A-11 two (143) and 07A-12 two (143 after the retarget). The ignored
test is the worker integration run, which now reports honestly: `cargo test
--locked -p csv-preview --test session -- --ignored worker_backend` passes
with `csv-worker.exe` built (CI builds the workspace before running it).

Before the 07A-13 triage the clippy baseline was 32 warning lines on
stderr (26 `upper_case_acronyms` in the Windows module + 2 per target
`large_enum_variant` in the protocol, plus per-target summary lines);
with the deliberate allows, `-- -D warnings` yields zero.

[measured] Evidence highlights:

- `register_plan_writes_disable_lowil_dword_and_unregister_removes_it` panics
  before the Low-IL fix (`registration must write the Low-IL opt-out as a
  DWORD`) and passes after; a foreign value under the same name survives.
- `head_only_stream_preview_returns_promptly` panics before the marshalling
  fix (an eager copy took 50.33 s for a 256 MiB slow source) and passes after;
  `a_host_stream_marshals_across_apartments` drives real COM marshalling.
- `worker_scan_over_piped_stdio_returns_a_report` proves the full worker scan
  still completes over pipes with `CREATE_NO_WINDOW` set.
- `field_beyond_the_byte_budget_stops_the_scan_as_partial` and
  `worker_budget_reports_partial_with_huge_field` fail before the budget
  (a 12 MB single field was buffered whole and reported complete) and pass
  after (bounded, partial with a per-field byte budget message).
- `panic_in_preview_does_not_abort_host` injects panics through all four
  containment shapes; before containment a panic unwinding out of an
  `extern "system"` function aborted the test binary (exit 0xC0000409).
- `window_class_registers_with_the_given_module` proves the class belongs to
  the given module rather than the `GetModuleHandleW(NULL)` fallback;
  `unload_is_refused_while_a_library_thread_is_alive` pins the unload gate.
- `register_refuses_when_hklm_shellex_exists` fails before (no HKCR read) and
  passes after for a machine-only, a user-hive and a clean registry; the CLI
  confirmation seam proves stdin-n never executes and stdin-y runs with the
  force flag.
- `unregister_removes_the_empty_8895_leaf` and `unregister_without_registration_is_a_no_op`
  fail before the 07A-08 deletion loop (residue assertion panic) and pass
  after; `SHDeleteKeyW` was probed live on a scratch HKCU key (delete → 0,
  missing → 2) before use.
- `spool_exact_cap_is_not_truncated`/`spool_one_byte_past_the_cap_is_truncated`
  (07A-09) fail before the one-byte probe (exact cap wrongly reported
  truncated) and pass after, with the probe result restored and ignored used
  for the fails-before run.
- `dpichanged_afterparent_rebuilds_the_font` (07A-10) drives the real
  window proc on a real HWND: before the 0x02E3 arm the font kept the stale
  16 px metrics; after, metrics match a font built from `GetDpiForWindow`.
- `click_focuses_the_pane_and_vks_compare_correctly` and
  `unhandled_keys_forward_to_the_host_frame` (07A-11) fail with the old proc
  (no focus arm, `b'f'` = VK_NUMPAD6 match, `S_FALSE` without forwarding) and
  pass after, with real focus checks and a fake `IPreviewHandlerFrame` counting
  references.
- `slow_stream_preview_still_returns_a_head` (07A-12) now drives the
  production `read_head` + `start_with_head` composition and the new counting
  reader proves the source flows into the spool exactly once; the hidden
  worker binary run evidences the old skip as `10 passed; 0 ignored` versus
  the new `1 ignored`.

[measured] Rust sources under `crates` total 11,927 lines across 34 files,
including tests.

## Tests that need real Windows

Cross-compiling proves the handler builds; it does not exercise Explorer. The
manual checks now required (see `docs/TESTING.md`) include:

- The preview handler end to end in the Explorer preview pane, including the
  Low-IL opt-out actually taking effect for the spool copy under `%TEMP%` and
  the `F` full scan not flashing a console window.
- COM apartment behaviour and streams that do not marshal (SMB, OneDrive
  placeholders); the fallback path is only reachable with such a stream.
- The FreeLibrary loop (preview → unload → release → preview again ×50) that
  proves unload safety; no unit test can model `FreeLibrary`.
- Peak worker memory on a huge-field fixture (the budget test bounds the
  behaviour logically; RSS is measured manually via `Get-Process`).
- Registration/unregistration conflicts, including a seeded machine-wide
  `ShellEx` handler with and without `--force`, and rollback under concurrent
  changes; visual layout, packaging and deployment.
- (XR-2) Drag Explorer between 100% and 200% monitors while a preview pane is
  open: the pane font must rescale (07A-10's decisive manual step).
- (XR-2) Click the pane and press F: the full scan starts (07A-11) and no key
  traps in the pane; the site/frame path is bound by Explorer's `SetSite`.
- Uninstall behaviour: after `--unregister` a fresh registration works with
  no stale leaves shadowing a machine-wide handler (07A-08's real-registry
  verification, previously item 8 of XR-2).
- The first CI run of the new workflow on the PR: `cargo deny` step, Linux
  job and the tag release job are pinned-action config that cannot execute
  from this box (locally verified: YAML parse, clippy gate, builds).

## Deviations from the fix-plan specs (also in each commit message)

- **07A-01:** Variant 1 as decided — register `DisableLowILProcessIsolation=1`
  (REG_DWORD) under our CLSID key; Variant 2 (spool to LocalLow) stays parked
  on branch `variant/07a-01-locallow-spool`. The registry layer carries typed
  values (`Value::Text`/`Value::Dword`, `ExpectedValue::Dword`).
- **07A-02:** `PreviewSession::start_with_head` and a `pub read_head` are
  factored out of `PreviewSession::start` (folding part of 07A-12's planned
  retargeting into this finding); a non-marshalable stream falls back to the
  previous bounded spool copy.
- **07A-03:** the spec cites `crates/csv-core/src/backend.rs`; the spawn lives
  at `crates/csv-preview/src/backend.rs` (same lines 91-98). The window flash
  itself is manual-only evidence.
- **07A-04:** the fixture `huge_field_is_truncated_with_a_warning` fed a 200 KB
  field with `max_field_chars=1024` (budget 4096 bytes) and expected a complete
  report; its field is now 3000 bytes so it still covers the post-hoc
  `LongField` truncation, and a sibling test covers the over-budget hard stop.
  The optional job-object variant was not implemented (the budget is the fix).
- **07A-05:** DllMain stays unwrapped (its body is a single atomic store); the
  waiter-thread spawn got the guarded Builder shape together with 07A-06.
- **07A-07:** `csv-viewer` gained a dependency on `csv-preview` for the
  registration plumbing; preflight read errors keep the old lenient behaviour
  (warn and proceed) instead of refusing. TESTING.md registration bullets were
  synced; the stream bullet is left for 07A-12.
- **07A-08:** `SHDeleteKeyW` was probed live on a scratch HKCU key while the
  implementation was being verified; the probe test was removed after the
  evidence, per the local no-forever-mutations rule. The stale `30`-count
  clippy "warning baseline" in the medium-batch note was a PowerShell stderr
  artifact; the true baseline was 26/26/2/2 grouped lines (recorded in
  `w07b.txt`), which 07A-13 then triaged away.
- **07A-09:** the spec's acceptance arithmetic counted the spool head and the
  rest separately; the cap counts the head too, so the exact-cap test uses a
  source length equal to `max_spool_bytes` (91-byte source for a 90 cap).
- **07A-10:** the automated test drives a real HWND through `SendMessageW`
  real message delivery (spec suggested a call-count fake); the decisive
  manual step (drag between 100%/200% monitors) is recorded above for XR-2.
- **07A-11:** ships **both** subsets — the VK/click-to-focus subset in
  `a25aa32` and the `IObjectWithSite`/frame-forwarding subset in `ed25177` —
  per the Manager ruling taken mid-session (the spec's open question was not
  in DECISIONS.md); mirrored from parquet-viewer L5.
- **07A-12:** the extraction the spec wanted already landed with 07A-02's
  `start_with_head`/`read_head` seam, so the finding collapsed into the
  retarget + honest skip exactly as the spec anticipated; the worker test is
  `#[ignore]`d by default and fails loudly when run explicitly without the
  worker (CI builds the workspace first).
- **07A-13:** the 26 acronym warnings are triaged with one whole-file
  `#![allow(clippy::upper_case_acronyms)]` (plus rationale) instead of 26
  per-item attributes — all 26 declarations mirror Windows SDK names, so one
  comment covers one rationale; the two `large_enum_variant`s carry per-enum
  comments as specified. The release job is tag-triggered only per the
  Manager ruling (no `workflow_dispatch`). The cargo-deny step, Linux job
  and release job reuse the shipped parquet-viewer pin set; first run
  happens on the PR.
- **07A-14:** Path B of C2 landed (template + optional section, NOTICE
  untouched); the GNU-runtime question resolved to "add notices" after the
  spec's requested `objdump` check measured a static MinGW-w64 runtime in
  GNU builds, with the THIRD_PARTY review note recording that shipped
  releases use the MSVC script (the C2-not-ruled "GNU vs MSVC" open question
  is answered administratively: releases are MSVC; the GNU path stays a
  documented build convenience with notices in place).

## Environment notes

- `core.autocrlf` must stay `true` in this checkout, or git records whole-file
  CRLF↔LF diffs. Files are stored LF and checked out CRLF.
- The `csv-core` fixtures were normalized to LF locally (removed and restored
  with `git checkout --`) to fix a pre-existing Windows-only failure
  (`crates/csv-core/tests/fixtures.rs::preserves_quoted_multiline_cells`, CRLF
  checkout versus LF blob). No repository change; the blob is already LF.

## Open issues (for the Manager)

- All 14 findings are implemented on this branch; nothing is parked except
  the two variant worktrees noted in DECISIONS.md (`07a-01-alt`,
  `07a-02-sta`).
- The PR's first CI run validates the new workflow parts that cannot run
  locally (cargo-deny step and its licenses-only `deny.toml` against the
  `bans` default, the Linux job, the tag release job). If the `bans` default
  rejects the three duplicate lockfile versions (`windows-sys` x3, `rustix`
  x2, `linux-raw-sys` x2 — all target-gated cargo deps), add an explicit
  `[bans]` section; the shipped parquet-viewer config passes with the same
  shape.
- AGENTS.md rule 7 still says "keep LICENSE and NOTICE unchanged"; per C2
  that line is estate-owned and the Manager updates it in her own time.
