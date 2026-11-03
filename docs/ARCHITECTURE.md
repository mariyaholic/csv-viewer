# CSV viewer and preview

## Proposed standalone desktop companion

See [Standalone architecture](STANDALONE-ARCHITECTURE.md) for the owner-requested
GUI proposal: no COM dependency, shared native presentation, separate worker-backed
format adapters and staged acceptance gates. This is design work, not a shipped GUI.
The proposal extends companion scope; existing Explorer security gates remain unchanged.

## Six fields

1. **One-line pitch:** [inferred] Inspect a CSV in Explorer or a standalone window with an explicit account of parsing and scan coverage.
2. **Who it's for:** [inferred] Anyone inspecting unfamiliar tabular exports, logs or datasets; a trading profile is optional.
3. **5-second demo shot:** [guess] Select a CSV and see its first rows, delimiter, columns, scanned row count and a clearly marked incomplete scan.
4. **Overlap check:** [inferred] Excel offers rich spreadsheet analysis; Notepad shows raw text; VS Code and its extensions offer editing and previews; Windows Explorer preview handlers and PowerToys Peek provide quick views where configured. Capabilities vary by version and installed extensions, so benchmark the selected setup before release.
5. **Reuse:** [measured] Ōsōji's [scan result](/workspace/desk/ava/repo-survey/targets/osoji/internal/scan/scan.go:19) records results and skip reasons; borrow that reviewable-result design after rights review, not deletion logic. [inferred] CSV parsing, preview integration and quality checks are new.
6. **Caveats:** [inferred] Windows preview integration and distribution are Windows-specific; parsing and counts can be ambiguous or incomplete. The optional market profile reports heuristics, not proof of fraud or authenticity; not financial advice. [measured] The brief reports such heuristics caught one fake market file ([BRIEF.md](/workspace/qol-portfolio/BRIEF.md:31)), not their accuracy in general. GitHub and winget binaries are “not code-signed yet” unless signing is approved; SmartScreen may warn.

## Edge over defaults

[measured] On FLUORITE-EYES Explorer uses Windows TXT Previewer for CSV, showing raw text; on the 2 GB fixture DuckDB counted 20,065,328 records in 0.67 s, while `wc -l` counted 20,069,342 physical lines and pandas lost leading zeros on a 20k sample ([COMPARISON.md](/workspace/qol-portfolio/COMPARISON.md:5), [results](/workspace/qol-portfolio/COMPARISON.md:14)). [documented] PowerToys Peek uses plain text; Excel's direct open can convert leading zeros and dates, although Excel was not exercised on the machine ([comparison](/workspace/qol-portfolio/COMPARISON.md:20)). VS Code's extension-based tables are an inference.

[inferred] **Build with narrowed edge:** an Explorer table preview preserving source cell text and warning about leading zeros, ambiguous dates, mixed types, boolean spellings and quoted multiline records—the changes Excel may silently make. Clearly mark partial scans and coverage. Use a proven parser; do not compete with DuckDB or qsv on speed, SQL or query power. No editing or SQL; trading checks are optional. A small companion viewer is optional, not a full analysis app.

## Architecture

- **Stack:** [inferred] Rust native COM preview-handler DLL and separate constrained worker, with an optional small companion viewer; Rust suits native COM interop and separates parsing from the Explorer-facing component. Select a maintained CSV parser after security and license review.
- **Modules:** [inferred] About screen showing “Made by Maria”; minimal preview handler, versioned worker protocol, bounded stream transfer, encoding/dialect detection and override, incremental parser, virtualized table, scan and results model, optional market profile and registration. Show first rows promptly; full counts and date ranges require an explicitly requested complete scan; a partial scan displays scanned records, byte coverage and incomplete status, never an exact total. Cancel on selection change and show timeout or worker failure. Parse quoted newlines and avoid executing formulas or opening links.
- **Key Windows APIs:** [inferred] `IInitializeWithStream` and `IPreviewHandler`, COM registration, child-window hosting and job/process controls. The handler forwards the supplied stream through bounded IPC or a private bounded temporary spool; it does not assume a filesystem path. Apply worker resource and time limits; a job object alone is not a security sandbox. Validate Explorer host bitness and isolation, including blocked files.
- **Data/config:** [inferred] Per-user settings for dialect and optional profile; source remains read-only and local. Date range requires a selected or confidently identified column and a specified interpretation; show errors, sampling and coverage. Optional market checks use user-mapped fields for constant spread, open equal to previous close, return-tail distribution and price plausibility; report thresholds, sample size and missing data. Reference comparisons require a dated, licensed, instrument-specific local baseline; otherwise show unavailable. Heuristics can misclassify data.
- **Packaging:** [inferred] Version-matched DLL and worker, optional small companion viewer and installer with opt-in per-user preview registration, warning before changing the registration and clean uninstaller that restores prior associations when safe. GitHub Releases with SHA256SUMS, then winget via manual Ava/Manager review; prepare optional Store MSIX, but validate preview-handler registration and provide standalone fallback if unsupported. Apache-2.0 `LICENSE`, SPDX headers and third-party `THIRD_PARTY` inventory; review provenance and licenses. [measured] Ava's supplied, unverified policy says Store-certified MSIX is re-signed with a Microsoft certificate; EXE/MSI submissions require trusted pre-signing and self-signed certificates are rejected ([BRIEF-v5.md](/workspace/qol-portfolio/BRIEF-v5.md:18)). GitHub/winget stay unsigned with “not code-signed yet” and SmartScreen caveat unless SignPath Foundation approves. [measured] The supplied [SignPath terms](https://signpath.org/terms) are not re-verified: developer libraries/tools likely qualify for the reputation exemption; apps require verifiable reputation, 2FA, CI-built binaries, manual release approval, a Code signing policy naming authors/reviewers/approvers, privacy statement, uninstaller and warning before system-setting changes; publisher displays SignPath Foundation; applying is the Manager's call ([BRIEF-v5.md](/workspace/qol-portfolio/BRIEF-v5.md:18), [BRIEF.md](/workspace/qol-portfolio/BRIEF.md:87)).
- **Tests:** [inferred] Test UTF encodings, dialect overrides, quoted multiline fields, malformed and huge records, formula-like text, cancellation, complete versus partial counts and date ranges. Test market checks on synthetic and genuine fixtures, false positives and absent baselines. On Windows test stream-only input, limits, worker crash, selection change, registration conflicts, upgrades and uninstall; compare alternatives with the same fixtures.
- **Size:** [guess] M–L, 10–18 developer days and 1,000–2,000 changed LOC, excluding baseline acquisition and external review.

## Licensing

[measured] The Manager requires Apache-2.0 for every project ([Amendment 10a](/workspace/qol-portfolio/BRIEF-v5.md:40)). Include the full Apache-2.0 text in `LICENSE`, use `SPDX-License-Identifier: Apache-2.0` on project source where practical, and open every README with “Made by Maria”. Each UI app includes an About screen with “Made by Maria”. Store copy, where applicable: “free and open source (Apache-2.0)”. The root `NOTICE` starts with the template (C2 ruling, `docs/fix-plan/DECISIONS.md`):

```text
CSV viewer and preview
Copyright 2026 Maria (https://github.com/mariyaholic)

Made by Maria. This product includes software developed by Maria.
```

Below the template, `NOTICE` may carry one optional, clearly titled third-party section; this repo keeps the two pointer lines to `THIRD_PARTY.md` in that section.

[inferred] MIT, BSD, ISC, Apache-2.0 and Zlib dependencies are compatible subject to their terms; retain third-party notices in `THIRD_PARTY` so the `NOTICE` template stays the exact notice head. Audit all code, transitive dependencies and assets; do not reuse GPL code such as mukunku/ParquetViewer. Rights and obligations still require review.

[measured] The [SignPath application draft](/workspace/qol-portfolio/SIGNPATH_APPLICATION_DRAFT.md:1) is draft only; a public repository and an existing public release are prerequisites and the Manager decides publication/application. Apache-2.0 preserves eligibility; no application or publishing is authorized by this architecture.

## Open questions

- [guess] Which specific warning rules best identify possible Excel conversions without implying they always occur?
- [guess] Which CSV dialects and encodings are required initially, and what limits are acceptable for a full scan?
- [guess] Can optional MSIX integration register the handler on the supported Windows versions without displacing another handler unexpectedly?
