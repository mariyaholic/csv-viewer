# Status

Made by Maria. Apache-2.0.

The CSV viewer, parser, bounded preview session, worker, protocol and Windows
Explorer handler are built. Unregistration removes matching owned values and
leaves shared registry keys in place; empty keys are expected residue. The
exclusive CLSID tree could be deleted, but is also retained. Rollback restores
prior values when its ownership check succeeds and does not delete keys.

[measured] This branch implements the Opus-review fix plan for this repository
(14 findings, one commit each), highest severity first. The two findings done so
far repair the two ways the Explorer preview pane failed before it could draw:
the handler now opts out of the Low integrity level that `prevhost.exe` imposes
(07A-01), and the host input stream is read on the calling thread only for a
bounded head while the rest is marshalled to the session's own spool thread, so
a large file no longer blocks the host apartment and is never copied whole into
RAM (07A-02). The remaining twelve findings (07A-03 … 07A-14) are not yet on
this branch.

## Commits (one per finding)

| Finding | Commit | Summary |
|---|---|---|
| 07A-01 | `987fb90` | opt the preview handler out of Low-IL process isolation; typed registry values (`REG_DWORD`) |
| 07A-02 | `8b4b7ae` | stream the host input to the session off the host apartment |

## Build and test

Commands run locally (Rust 1.98.1; the host toolchain is
`x86_64-pc-windows-gnu`, so the `#[cfg(windows)]` handler code and the COM
marshalling test run for real; `core.autocrlf=true`):

```text
cargo fmt --check                                     # exit 0
cargo clippy --locked --all-targets                   # exit 0; 32 pre-existing warning lines
cargo build --locked                                  # ok
cargo build --locked --target x86_64-pc-windows-gnu   # ok
cargo test --locked                                   # 122 passed, 0 failed
```

Baseline on `main` was 119 passing. 07A-01 added one test (120); 07A-02 added
two (122).

[measured] `register_plan_writes_disable_lowil_dword_and_unregister_removes_it`
panics before the fix (with the plan entry removed: "registration must write the
Low-IL opt-out as a DWORD") and passes after. A foreign value stored under the
same name is left in place by unregistration.

[measured] `head_only_stream_preview_returns_promptly` panics before the fix
(the pre-fix eager pipeline took 50.33s to read a 256 MiB slow source on the
calling thread) and passes after (the head-only read returns in milliseconds).
`a_host_stream_marshals_across_apartments` drives real COM
(`CoMarshalInterThreadInterfaceInStream` then `CoGetInterfaceAndReleaseStream`
on a second thread) over an `SHCreateMemStream` payload of all 256 byte values.

[measured] Rust sources under `crates` total 10,814 lines across 31 files,
including tests.

## Tests that need real Windows

Cross-compiling proves the handler builds; it does not exercise Explorer. Once
the remaining findings land, these still need a real host (see
`docs/TESTING.md`):

- The preview handler end to end in the Explorer preview pane, including the
  Low-IL opt-out actually taking effect for the spool copy under `%TEMP%`.
- COM apartment behaviour and streams that do not marshal (SMB, OneDrive
  placeholders); the fallback path is only reachable with such a stream.
- Registration/unregistration conflicts and rollback under concurrent changes;
  visual layout, packaging and deployment.

## Deviations from the fix-plan specs (also in each commit message)

- **07A-01:** Variant 1 as decided — register
  `DisableLowILProcessIsolation=1` (REG_DWORD) under our CLSID key; Variant 2
  (spool to LocalLow) stays parked on branch `variant/07a-01-locallow-spool`.
  The registry layer therefore carries typed values (`Value::Text`/`Value::Dword`,
  `ExpectedValue::Dword`), and `Registry::read` returns `Option<Value>`.
- **07A-02:** `PreviewSession::start_with_head` and a `pub read_head` are
  factored out of `PreviewSession::start` so the Windows path can supply the
  already-read head plus a reader that is only safe on another thread; this is
  folded into 07A-02 because its promptness test needs it (07A-12 will retarget
  the remaining stale tests later). A stream that cannot be marshalled falls back
  to the previous bounded spool copy on the host thread.

## Environment notes

- `core.autocrlf` must stay `true` in this checkout, or git records whole-file
  CRLF↔LF diffs. Files are stored LF and checked out CRLF.
- The `csv-core` fixtures were normalized to LF locally (removed and restored
  with `git checkout --`) to fix a pre-existing Windows-only failure
  (`crates/csv-core/tests/fixtures.rs::preserves_quoted_multiline_cells`, CRLF
  checkout versus LF blob). No repository change; the blob is already LF.

## Open issues (for the Manager)

- This branch carries the two highest-severity findings only. Direction is
  requested before continuing with 07A-03 … 07A-14.
- All rulings for this repository are recorded in `fix-plan/DECISIONS.md`
  (C1 uninstall, C2 NOTICE, 07A-01 variant, 07A-02 marshalling, 07A-13 clippy
  triage); no blocking question remains.
