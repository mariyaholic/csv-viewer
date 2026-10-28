# Roadmap

Made by Maria. Apache-2.0.

v1 deliberately stops at the reviewed preview and viewer described in
[README.md](README.md). The following are **not** implemented.

## Packaging and distribution

- Version-matched installer with opt-in per-user preview registration, a
  warning before changing associations, and clean uninstall that restores a
  previous handler only when safe.
- MSIX/Store packaging and validation of preview-handler registration; a
  standalone fallback when MSIX cannot register the handler.
- GitHub Releases with `SHA256SUMS`, winget submission and Store submission.
  Publishing requires the Manager's decision; none is authorized here.
- Code signing (SignPath or another approved route). Until then binaries are
  “not code-signed yet” and SmartScreen may warn.
- 32-bit build and ARM64 build. v1 targets x64 Windows.

## Runtime and isolation

- OS-level worker memory/CPU limits via a job object or restricted token, with
  tests proving the worker still fails safely. Logical record/byte/time caps
  exist today.
- Full COM sandbox/isolation review against supported Windows versions,
  including blocked files and network shares.
- Cross-apartment `IStream` marshalling or a caller-thread spool design, after
  the Windows stream-lifetime tests in [TESTING.md](TESTING.md).

## Parsing and preview

- Additional encodings (for example Shift_JIS, GB18030, ISO-8859-1) and
  user-visible encoding labels.
- More dialects (custom quote/escape, comment lines, fixed-width input) and
  clearer confidence diagnostics.
- Richer preview UI (scrollable GDI table or WebView2), without executing
  formulas or opening links.
- Faster preview for very large files and configurable head size.
- Date-range selection UI inside the viewer; v1 exposes it on the command line
  and in the core model.
- Additional review heuristics only after measuring false positives against
  real fixtures.

## Market profile

- Local baseline acquisition/import workflow with dated, licensed,
  instrument-specific provenance and a review screen.
- More robust tail statistics with larger samples and explicit sampling
  coverage.
- Optional MT5/MetaTrader **read-only** import, if rights and API constraints
  are reviewed. No order placement, modification, closing or other trading
  calls will be added; no affiliation with MetaQuotes is claimed.
- Clearer language that heuristics are not proof of fraud or authenticity and
  not financial advice.

## Explicit non-goals

- Editing CSV files, SQL, query engines, spreadsheet calculation, row
  insertion/deletion, merging or export.
- Competing with DuckDB/qsv on speed or query power.
- Silent type conversion or formula execution.
- Telemetry, network publishing or uploads.
