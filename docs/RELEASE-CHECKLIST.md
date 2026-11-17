# Standalone public-beta release checklist

Made by Maria. Free and open source (Apache-2.0).

Status: preparation only. No release, tag, repository-visibility change or
publication is authorized by this document.

## Scope

The first beta candidate is **Windows x64 MSVC standalone CSV Viewer**.
It includes csv-desktop.exe, csv-worker.exe, the offline guide, three optional
OFL font assets and the project/dependency notices. It excludes the Explorer
COM DLL, terminal viewer, registration scripts, settings, source CSVs and
developer caches. Explorer integration needs its own native acceptance.

The app is read-only. It does not evaluate formulas or open source links.
Clipboard copies and reports can still contain private source data. Worker
bounds do not make the app a security sandbox. Binaries remain unsigned.

## Build a candidate

Use a clean checkout of the reviewed commit, Windows x64, Rust 1.85.0 with the
x86_64-pc-windows-msvc target, Windows SDK resource compiler and Python 3.
Dependencies must be available from the locked Cargo source graph.

```powershell
./scripts/build-desktop.ps1 -OutputDirectory dist/candidate-1
python scripts/package-desktop.py --verify dist/candidate-1/csv-desktop-release-windows-x64-msvc.zip
Get-FileHash dist/candidate-1/csv-desktop-release-windows-x64-msvc.zip -Algorithm SHA256
```

The builder checks the offline guide and dependency notices before packaging.
The ZIP has an exact allowlist and an internal SHA256SUMS for every payload file,
including BUILD.json. A sibling .zip.sha256 covers the entire ZIP. Output names
must be new: use a new candidate directory rather than overwriting earlier evidence.
The packager checks x64 PE headers, not code signing, import safety or toolchain
provenance. BUILD.json records the commit/profile supplied by the builder.
Do not package unrelated binaries with a misleading source label.

The GNU shell script remains a developer convenience. Do not distribute that
payload as the MSVC beta; GNU runtime terms need separate review.

Tags now stage a **review-only Actions artifact**, after Linux, Windows and
license checks pass. They do not create or publish a GitHub Release. Publishing
requires a separate owner decision after the checklist below. PR CI exercises
the ZIP format and extracted debug app but does not certify a release build.

## Gates before any public exposure

- [ ] Review and merge the preparation PR only after its exact head passes CI.
- [ ] Record the exact candidate source commit, build toolchain, ZIP SHA256,
  dependency-notice check and fresh vulnerability-database result.
- [ ] Inspect the actual release PE imports/runtime requirements and any
  bundled runtime redistribution terms.
- [ ] Verify fresh full-history screening and owner approval of repository
  history, old work-planning files, commit identities and screenshot contents.
  See [audit scope and limits](RELEASE-AUDIT-2026-11.md).
- [ ] Verify the private vulnerability-reporting route is actually available
  to an outside reporter before advertising it.
- [ ] Decide signing/distribution policy and state the unsigned-build warning.
  Do not instruct users to disable Windows security protection.
- [ ] Complete the clean-machine acceptance below and record evidence.
- [ ] Owner separately approves repository visibility, exact release/version,
  final assets and publication. No tag or public release is implied by a green PR.

## Clean Windows acceptance for the exact ZIP

Run on a clean Windows x64 VM/account without Rust, Cargo, developer PATH
entries or an existing CSV Viewer installation. Keep network access off during
runtime checks. Record Windows build, display scaling and candidate SHA256.

1. Verify the outer checksum. Extract the whole ZIP into a folder with spaces
   and non-ASCII characters. Never run an EXE directly inside the archive.
   Confirm both executables, fonts, font licenses, offline help and notices.
2. Launch by double-click as a standard user. Check missing-runtime errors,
   startup warning behavior and no administrator/registry prompt.
3. Open/drop synthetic UTF-8/BOM, UTF-16 and Windows-1252 samples. Verify leading
   zeros, quoted newlines, formula-like values and URLs remain literal text.
   Compare source hashes before and after.
4. Exercise next/previous, literal search beyond row 200, absent search,
   Clear search, copy shown row and the displayed coverage/truncation labels.
5. Cancel an active operation, open another file, reload, close while loading,
   relaunch repeatedly and remove the worker from a duplicate test extraction.
   Failure must be clear and recoverable. Restore only that test copy.
6. Confirm first launch and subsequent launches never reopen private file
   history. Check theme/window restoration, unsupported-settings recovery,
   F1, every Help topic and offline guide without network.
7. Test physical Tab/Shift+Tab, arrows, shortcuts, Narrator/UIA, high contrast,
   100/150/200% DPI and two-monitor movement. Record limitations, not just pass/fail.
8. Check temporary-spool cleanup, no unexpected outbound network requests,
   no source modification and no Explorer registration changes.
9. Repeat against the **release** ZIP, not only the debug CI payload.
   Archive screenshots/logs only after removing private paths and data.

A clean developer CI runner is not a clean end-user machine. Wine and posted
Win32 messages do not replace physical input, accessibility or monitor checks.
