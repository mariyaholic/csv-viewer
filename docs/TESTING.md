# Testing

Made by Maria. Apache-2.0.

## Linux (this machine)

Run offline with `CARGO_NET_OFFLINE=true cargo test --offline --workspace`.
Run `cargo fmt --check`, `cargo clippy --offline --all-targets`, and
`cargo build --offline --workspace`. Cross-compile and lint with target
`x86_64-pc-windows-gnu`; cross-compilation does not exercise Windows behavior.

Expected coverage:

- Unit tests for types, encoding detection/decoding, dialect/header inference,
  warnings, scan limits/coverage, dates, market checks, settings, table
  virtualization, protocol framing and worker path/cap checks.
- Fixture tests for UTF-8, UTF-8 BOM, UTF-16LE/BE BOM, Windows-1252,
  comma/semicolon/tab/pipe, quoted multiline, ragged rows, empty files,
  single-column files, CRLF, unterminated quotes, date ranges and market data.
- Golden report test in `crates/csv-core/tests/golden/golden_report.json`.
- Worker integration tests over real pipes: handshake, scan, truncated source,
  path outside spool, size mismatch, wrong protocol version, and equality with
  an in-process scan.
- Preview tests: immediate head preview, spool readiness, spool cap, explicit
  full scan, cancellation, timeout, missing worker and worker-backed scan.
- Viewer tests: argument parsing, request construction, key handling, table
  scrolling, About text and date-range forwarding.

Regenerate the parser golden deliberately:

```sh
UPDATE_GOLDEN=1 cargo test -p csvview-core --test golden
```

Review the diff before committing. Do not regenerate to hide an unexpected
change.

## Manual Linux checks

```sh
cargo run -q -p csv-viewer -- --render --full \
  crates/csv-core/tests/fixtures/golden.csv
cargo run -q -p csv-viewer -- crates/csv-core/tests/fixtures/golden.csv
```

In the TUI check: arrows/PageUp/PageDown, `w` warnings, `d` details,
`a` About (shows “Made by Maria”), `f` explicit full scan, `q` quit.
Resize the terminal and confirm the table stays virtualized. In `--render`
mode confirm a partial preview has no “Total records” line.

## Real Windows testing required

The following must be exercised on a real Windows PC before release:

1. **Build/packaging**
   - Build x64 DLL and worker with a supported MSVC toolchain; confirm
     version-matched DLL/worker and that both are included together.
   - Check DLL exports (`DllGetClassObject`, `DllCanUnloadNow`,
     `DllRegisterServer`, `DllUnregisterServer`) and x64 bitness.
   - Test on a clean Windows VM without a Rust toolchain.
2. **COM / Explorer**
   - Register per-user, open a `.csv` in Explorer, confirm the handler loads
     and the preview appears in the preview pane.
   - Test Explorer 32-bit and 64-bit isolation; confirm the DLL bitness
     matches the host.
   - Test a stream-only source (no filesystem path), a blocked file
     (Mark-of-the-Web/Zone.Identifier), a read-only file and a file on a
     network share.
   - Verify `IInitializeWithStream` rejects write modes and handles `IStream`
     `Read`/`Stat` failures.
   - **Apartment/stream lifetime:** the caller copies the stream to a bounded
     temporary file before passing a file handle to the spool thread. Verify COM
     calls and releases occur on the owning apartment, test read errors and a
     slow stream. The synchronous copy may delay Explorer up to the spool cap.
   - Change selection while a full scan is running: the old session must
     cancel and the new preview must appear.
   - Kill `csv-worker.exe` mid-scan and confirm a failure/timeout message, no
     hang and no exact total.
   - Test the 120 s full-scan timeout and a worker that never responds.
   - Test DPI scaling, multiple monitors, window resize, focus, and the `F`
     full-scan key.
3. **Registration**
   - Confirm `DllRegisterServer` writes only
     `HKCU\Software\Classes\CLSID\{8f6a0f5e-3c4b-4a2e-9d7f-5b1c2e3a4f60}` and
     the `.csv`/`SystemFileAssociations\.csv` `ShellEx` values, with
     `ThreadingModel=Apartment`, the prevhost `AppID`, `DisplayName`, and
     the per-user `PreviewHandlers` value.
   - Install over a different existing CSV preview handler: registration must
     refuse (the installer must show the warning and ask first).
   - Unregister removes only values still matching this handler; a handler
     installed later and foreign values must survive. Shared keys are never
     deleted, so empty keys are expected residue. The exclusive CLSID tree
     could be deleted, but is also left in place. Test rollback on failed
     writes, including prior values and concurrent changes; shared keys remain.
   - Test upgrade in place and uninstall; confirm Explorer restores the prior
     handler when safe.
   - Test optional MSIX registration and the standalone fallback if preview
     registration is unsupported. Installer/MSIX is deferred (ROADMAP).
4. **Parsing and UI on Windows**
   - UTF-8/BOM, UTF-16LE/BE BOM, Windows-1252, comma/semicolon/tab/pipe,
     CRLF and quoted embedded newlines.
   - Leading zeros, ambiguous DMY/MDY dates, long numbers, boolean spellings,
     formula-like text and links render as text; no formula execution, no link
     opening.
   - Huge file: preview appears promptly; full scan can be cancelled and
     reports partial coverage with no total.
   - A file larger than the spool cap: preview still works, full scan is
     partial and marked truncated.
   - Market profile fixtures on Windows produce the same findings as Linux.
5. **Resource isolation**
   - Job-object/process memory limits are not implemented in v1. Verify the
     worker's logical record/byte/time caps and consider adding an OS job
     object; a job object alone is not a security sandbox.
   - Confirm no network access and no writes outside the private spool.

## Not tested on Linux

- Win32 registry calls, COM vtables, GDI drawing, window messages, Explorer
  hosting and any MSIX/installer behaviour.
- The `csvview_windows_check` cfg only type-checks the Windows module on Linux;
  it does not link or execute it.
