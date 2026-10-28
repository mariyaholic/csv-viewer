Made by Maria

# CSV viewer and preview

Inspect a CSV in Explorer or a standalone window with an explicit account of parsing and scan coverage.

This is free and open source (Apache-2.0). See [LICENSE](LICENSE) and [NOTICE](NOTICE).

Status: v1 implementation, Linux build and tests passing. Windows COM code is
implemented behind `cfg(windows)` but has **not** been compiled or exercised on
a real Windows target on this machine. Releases are not code-signed yet.

## What v1 does

- **Explorer preview handler (Windows, opt-in):** reads the stream Explorer
  supplies, never assumes a filesystem path, and renders a text preview with
  the source cells unchanged. Supports `IInitializeWithStream` and
  `IPreviewHandler`; registration is per-user under
  `HKCU\Software\Classes`.
- **Constrained worker:** a separate `csv-worker` process receives a path
  inside a private spool directory plus an exact byte count. The protocol is
  versioned and length-prefixed with a bounded frame size. Worker and request
  limits cap records, bytes and time.
- **Fast first screen, explicit full scan:** the handler keeps a bounded head
  (256 KiB) for the first preview. A full scan is only run when the user asks
  for it (F in the preview window, `--full` or `f` in the viewer).
- **Honest coverage:** a partial scan reports scanned records, bytes read and
  an incomplete reason. It never reports an exact total. Only a complete scan
  sets `total_records`.
- **Encoding and dialect detection with overrides:** UTF-8, UTF-16LE,
  UTF-16BE and Windows-1252; comma, semicolon, tab and pipe delimiters; quoted
  fields and embedded newlines.
- **Review notes, not conversions:** leading zeros, ambiguous day/month dates,
  mixed types, boolean spellings, long numbers, formula-like text, link-like
  text, quoted multiline fields, ragged rows, duplicate header names and
  replacement errors are reported with bounded samples and counts.
- **Date ranges only on request:** a 1-based column and an explicit `ymd`,
  `dmy` or `mdy` interpretation are required, and a range is only shown after
  a complete scan.
- **Optional market heuristics:** user-mapped OHLC fields can report constant
  spread, open equal to previous close, return tails, price plausibility and
  missing data, with thresholds and sample sizes. Baseline comparison is
  unavailable unless a dated, licensed, instrument-specific local baseline is
  supplied. Nothing is written and no trading API is called.
- **Companion viewer:** a small cross-platform terminal viewer with a
  virtualized table, review notes, scan details and an About screen.

## Build and test

Requires Rust 1.85. No Windows target or cross-compiler is needed for the
Linux build; the Windows module is excluded.

```sh
cargo build --workspace
cargo test --workspace
```

Type-check the Windows-only module on a non-Windows host (no link, no run):

```sh
RUSTFLAGS='--cfg csvview_windows_check' cargo check -p csv-preview
```

## Use

```sh
# Plain-text preview (also used automatically when stdout is not a terminal)
cargo run -p csv-viewer -- --render path/to/file.csv

# Interactive viewer: arrows scroll, PgUp/PgDn, w warnings, d details,
# a About, f explicit full scan, q quit.
cargo run -p csv-viewer -- path/to/file.csv

# Complete scan with an explicit date range (1-based column)
cargo run -p csv-viewer -- --render --full \
  --date-column 1 --date-format ymd path/to/file.csv

# Overrides
cargo run -p csv-viewer -- --render --delimiter ';' --encoding windows-1252 \
  --header present path/to/file.csv
```

Settings are per-user JSON (`%APPDATA%\csv-viewer\settings.json` on Windows,
`$XDG_CONFIG_HOME/csv-viewer/settings.json` or
`~/.config/csv-viewer/settings.json` elsewhere). `CSV_VIEWER_SETTINGS` and
`--settings` select an explicit file. `CSV_VIEWER_WORKER` overrides the worker
path used by embedders; the COM handler looks for `csv-worker.exe` next to the
DLL. Settings hold dialect overrides, preview
size and an optional market field mapping. They never contain credentials; do
not put any in them.

## Limits and behaviour

- Preview head: 256 KiB, 200 rows by default; preview cell text is capped at
  1 MiB per report so the worker frame stays bounded.
- Full scan: 100,000,000 records, 4 GiB source, 120 s by default; fields are
  capped at 4,096 display characters and columns at 1,024. Reaching a limit
  makes the result partial.
- Spool: private temporary directory, 4 GiB cap. If the source is larger, the
  spool is truncated and scans are partial; the source itself is untouched.
- Partial byte coverage is estimated at record boundaries; for multi-byte
  encodings it is interpolated, not an exact byte count. Record counts are
  exact for the records parsed, and a complete scan has an exact byte total.
- The source is opened read-only. Labels are treated as text: formulas are
  never evaluated and links are never opened.
- No network access, no telemetry, no uploads. Network use is not required at
  runtime.
- MT5/MetaTrader integration is **not** included. Any future market-data
  reading would be strictly read-only: no order placement, modification,
  closing or other trading calls. CSV viewer and preview is not affiliated
  with MetaQuotes; MT5 is a MetaQuotes trademark.
- Market findings are heuristics, not proof of fraud or authenticity and not
  financial advice.

## Windows status

The DLL/worker, registry plan and COM implementation live in
`crates/csv-preview` behind `cfg(windows)`. They require a 64-bit Windows build
and real Explorer testing; see [TESTING.md](TESTING.md) and
[STATUS.md](STATUS.md). Registration is opt-in, warns before changing a
preview association, and unregisters only values that still point to this
handler.

See [ROADMAP.md](ROADMAP.md) for deliberately deferred work and
[THIRD_PARTY.md](THIRD_PARTY.md) for the dependency inventory.
