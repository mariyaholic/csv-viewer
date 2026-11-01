# Status

Made by Maria. Apache-2.0.

The CSV viewer, parser, bounded preview session, worker, protocol and Windows
Explorer handler are built. Unregistration removes matching owned values and
leaves shared registry keys in place; empty keys are expected residue. The
exclusive CLSID tree could be deleted, but is also retained. Rollback restores
prior values when its ownership check succeeds and does not delete keys.

[measured] This branch implements the Opus-review fix plan for this repository
(14 findings, one commit each), highest severity first. Seven findings are
done: both highs (07A-01 Low-IL opt-out, 07A-02 stream marshalling) and all
five mediums (07A-03 console flash, 07A-04 per-field memory bound, 07A-05
panic containment, 07A-06 thread/class lifetimes, 07A-07 visible registration
warning plus HKCR preflight with `--register`/`--unregister`/`--force`). The
lows (07A-08 … 07A-14) are not on this branch yet.

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

## Build and test

Commands run locally (Rust 1.98.1; the host toolchain is
`x86_64-pc-windows-gnu`, so the `#[cfg(windows)]` handler code and the COM
tests run for real; `core.autocrlf=true`):

```text
cargo fmt --check                                     # exit 0
cargo clippy --locked --all-targets                   # exit 0; 31 pre-existing warning lines
cargo build --locked                                  # ok
cargo build --locked --target x86_64-pc-windows-gnu   # ok
cargo test --locked                                   # 135 passed, 0 failed
```

Baseline on `main` was 119 passing; 07A-01 added one test (120), 07A-02 two
(122), 07A-03 one (123), 07A-04 two (125), 07A-05 one (126), 07A-06 three
(129) and 07A-07 six (135).

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

[measured] Rust sources under `crates` total 11,730 lines across 34 files,
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

## Environment notes

- `core.autocrlf` must stay `true` in this checkout, or git records whole-file
  CRLF↔LF diffs. Files are stored LF and checked out CRLF.
- The `csv-core` fixtures were normalized to LF locally (removed and restored
  with `git checkout --`) to fix a pre-existing Windows-only failure
  (`crates/csv-core/tests/fixtures.rs::preserves_quoted_multiline_cells`, CRLF
  checkout versus LF blob). No repository change; the blob is already LF.

## Open issues (for the Manager)

- Remaining findings are the lows: 07A-08 (uninstall residue, C1 ruling),
  07A-09 (rides `write_spool`, same function as 07A-02), 07A-10, 07A-11,
  07A-12 (retargets stale tests, includes the TESTING.md stream bullet),
  07A-13 (clippy triage then `-D warnings`), 07A-14 (C2 NOTICE rule).
- All rulings for this repository are recorded in `fix-plan/DECISIONS.md`
  (C1 uninstall, C2 NOTICE, 07A-01 variant, 07A-02 marshalling, 07A-13 clippy
  triage); no blocking question remains.
