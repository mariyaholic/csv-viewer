# Status

Made by Maria. Apache-2.0. v1 implementation, 2026-10-28.

## What is built

- `csvview-core`: encoding detection/decoding (UTF-8, UTF-8 BOM, UTF-16LE/BE,
  Windows-1252), delimiter/header inference with overrides, streaming CSV
  parsing with quoted multiline fields and an unterminated-quote check,
  bounded record/byte/time scans, explicit coverage, review warnings, date
  ranges with explicit interpretation, optional market heuristics, per-user
  settings, virtualized table model and plain-text rendering.
- `csv-protocol`: versioned worker protocol with 4-byte length framing, JSON
  messages, a 16 MiB frame cap and path-inside-spool source specs.
- `csv-worker`: separate scan worker with handshake, progress, cancellation,
  worker-side caps and spool path validation (regular file, exact size, no
  symlink, inside the private spool).
- `csv-preview`: immediate bounded head preview, private bounded spool,
  explicit full-scan sessions, timeout/cancel/failure states, in-process and
  worker-backed scan backends, pure registration plan and a Windows-only
  COM preview handler (`IInitializeWithStream`, `IPreviewHandler`, class
  factory, GDI text window, per-user registration/unregistration).
- `csv-viewer`: terminal companion viewer with virtualized table, warnings,
  scan details, date-range flags, explicit full scan, About screen showing
  “Made by Maria”, and a plain-text render mode.

v1 deliberately does **not** include an installer, MSIX, winget/GitHub
publication, code signing, 32-bit/ARM builds, WebView2, editing, SQL, formula
execution, link opening, telemetry or network access. See `ROADMAP.md`.

## Commands and results

Run from the repository root:

```sh
cargo build --workspace
```

Result: `Finished dev profile` — success, no warnings.

```sh
cargo test --workspace
```

Result: **112 tests passed, 0 failed, 0 ignored** across the workspace test
targets (core unit/fixture/golden/market/warning tests, protocol tests, worker
unit and pipe integration tests, preview session/backend tests, viewer tests,
and doc tests).

```sh
cargo build --release --workspace
```

Result: `Finished release profile` — success.

```sh
RUSTFLAGS='--cfg csvview_windows_check' cargo check -p csv-preview
```

Result: success. This **type-checks** the Windows-only module on Linux; it does
not link, run or validate Win32/COM behaviour. The default `cargo test
--workspace` does not include the Windows module, so it links normally.

Manual smoke check:

```sh
cargo run -q -p csv-viewer -- --render --full \
  crates/csv-core/tests/fixtures/golden.csv
```

Result: complete scan, 3 records, 208/208 bytes, warnings for leading zeros,
long number, ambiguous dates, boolean spellings, formula-like text, link-like
text and quoted multiline text. `--date-column`/`--date-format` prints a
computed range only when the scan is complete.

Golden regeneration (deliberate review required):

```sh
UPDATE_GOLDEN=1 cargo test -p csvview-core --test golden
```

## Size

- 9,793 lines of Rust under `crates/` (library, binaries, unit and integration
  tests), up from the empty scaffold.
- 5 crates, 17 parser/market fixtures, 1 reviewed JSON golden report.
- Architecture estimate was 1,000–2,000 changed LOC excluding review; the
  implementation is larger because it includes the worker, preview session,
  TUI, Windows COM/registration layer and tests.

## Tests requiring real Windows

None of these have been run here. The full checklist is in `TESTING.md`:

- MSVC x64 build, DLL exports and bitness, version-matched DLL/worker.
- Explorer preview registration per-user, conflict refusal, unregister only
  when the value still points to this handler, upgrade/uninstall.
- Stream-only input, blocked/Mark-of-the-Web files, network shares, read-only
  files, Explorer 32/64-bit isolation.
- COM apartment and `IStream` lifetime across the bounded spool thread; if the
  host stream is apartment-bound this design must marshal it or spool on the
  caller.
- Selection-change cancellation, worker crash, worker hang/timeout, large
  files, DPI/multi-monitor/resize/focus, `F` full scan key.
- GDI painting/font/brush lifetime and child-window hosting.
- Optional MSIX registration and standalone fallback (not implemented).
- Registry writes/reads, job-object/resource isolation.

## Open issues and caveats

1. **Windows layer is unverified.** The `csvview_windows_check` cfg type-checks
   the module on Linux, but no Windows target/toolchain is installed and no COM
   registration or Explorer host has been exercised. Treat the DLL as
   development code until `TESTING.md` passes.
2. **Stream apartment lifetime.** `StreamReader` is moved to the spool thread.
   Explorer streams may be apartment-bound; real-Windows tests must confirm or
   force GIT marshalling/caller-thread spooling.
3. **No installer/MSIX/winget/signing.** `DllRegisterServer` is present, but the
   opt-in installer warning flow, prior-handler restore and clean uninstaller
   are future work. Registration refuses to replace a conflicting handler, but
   only the callers can show the user warning.
4. **No OS memory cap.** Worker limits are logical (records/bytes/time). A job
   object or restricted token is not implemented; a job object alone is not a
   security sandbox.
5. **x64 Windows only** in practice; `SetWindowLongPtrW` and the planned
   packaging assume x64. 32-bit/ARM are roadmap items.
6. **Large single fields.** Source/spool caps bound total input, but the `csv`
   crate can allocate a very large single record before post-parse checks. OS
   memory limits and a stricter field-size reader are future work.
7. **Conservative limits.** A file that ends exactly at the record limit may be
   reported partial rather than complete. This is intentional: never claim a
   total without proving EOF.
8. **CRLF line numbers** use a small display-only adjustment because the csv
   crate's line counter was observed one low for CRLF files; preview record
   numbers are the authoritative row identity.
9. **Encoding/locale limits.** Only UTF-8, UTF-16LE/BE and Windows-1252 are
   detected; decimal-comma dialect detection is heuristic and overridable.
10. **Market profile** needs a settings JSON with mapped fields/instrument for
    baseline comparison; there is no import/selection UI. Baselines must be
    dated, licensed, instrument-specific and local. Findings are heuristics,
    not proof of fraud/authenticity and not financial advice. No MT5/trading
    calls exist.
11. **Third-party review.** `unicode-ident` carries the permissive Unicode-3.0
    data licence in addition to MIT/Apache (build-time only). See
    `THIRD_PARTY.md` for the release-review note. No GPL-only dependency is
    used.
12. **No network publishing** was performed. All commits are local; no git
    remote, push, upload, telemetry or runtime network use.
