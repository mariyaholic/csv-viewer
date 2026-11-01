# XR-2 — Cross-repo: manual Windows checklist per repo (this PC is the acceptance bench)

- Applies to: all four repos.
- This is a *manual runbook*, not code. Every item points at the finding it verifies; run each repo's block **after that repo's code fixes land** (dependencies noted inline). One session per repo is enough; record pass/fail in the repo's `TESTING.md` section (the repos each expect one per the architecture).
- Not a review finding; track as XR-2. Size: M (execution time, not difficulty) · No fixer model required — the estate executes these.
- Manager sign-off: sign-off applies per-code-fix, not here; this spec exists so the checklists are written once and run honestly.

## csv-viewer (after 07A-01…07 fixes)
1. **Low-IL temp path (07A-01):** register the DLL; select a `.csv` in Explorer. Preview renders. In Process Explorer verify `prevhost.exe`'s integrity level (Low unless the fix chose local-spool/Low retention path — note which).
2. **Blocking copy (07A-02):** select a ≥ 2 GB CSV; first paint < 1-2 s; change selection mid-scan → new file previews immediately; `%TEMP%` grows at most head+spool as designed, not 2×4 GiB.
3. **Console flash (07A-03):** press `F` for the full-scan refresh; no console window appears (~1 s check).
4. **Worker RAM (07A-04 – optional targeted test, if a fixture exists):** scan a crafted 200 MB `"`-bomb file; peak RSS of `csv-worker.exe` stays under the budget the fix set (Process Explorer or `Get-Process`).
5. **Panic containment (07A-05):** run a debug build with a deliberately injected panic in `refresh_text` (as the finding's verify plan describes); prevhost stays alive; pane shows the error text.
6. **DLL unload/window class (07A-06):** preview → F → change selection → release in a loop (the finding's host harness or a selection-arrows stress) ×50; no crash on the 2nd+ scan.
7. **Conflict preflight (07A-07):** create an `HKLM` `.csv\ShellEx\{8895…}` value (as data, not by installing a competitor); run the register path; warning appears and refuses without `--force`.
8. **Residue (07A-08):** after registering and unregistering ours with an HKLM handler present, the original handler previews again (only if the C1 ruling adopted the leaf-delete policy).
9. **Exact-cap (07A-09):** preview a file whose size == the spool cap exactly; no "partial" marker unless genuinely over.
10. **DPI (07A-10):** move Explorer between 100% and 200% monitors; pane font rescales.
11. **Keys/focus (07A-11):** click the pane, press `F` — triggers; `Tab` in/out does not trap focus (with site/frame work if 07A-11's second half was done).

## parquet-viewer (after H1, M1, M2, L1-L5 fixes)
1. **Rendering (H1):** preview `tests/fixtures/trades.parquet` — every schema row and data line breaks on its own line.
2. **Error text (H1):** preview an invalid file — multi-line error pane.
3. **64 MiB / streaming (M1):** preview a ≥ 200 MiB parquet; if the marshalling fix shipped, it previews; else the README cap text is visible and the error is clear. SMB-share file + move selection mid-load → responsive.
4. **Temp leak (M2):** 20 quick selection changes on a large fixture (`%TEMP%\parquet-preview-*.tmp` empties — M2's acceptance; also a 512 MiB-job-trip file for completeness).
5. **Takeover backup (L1):** install a foreign `.parquet` ShellEx id, force re-register, uninstall — foreign handler's association restored.
6. **Order (L2):** induce a registry-write failure while registering (delete the CLSID mid-run is awkward — only run if the test double needs a live proof); association unchanged.
7. **No stray kill (L3):** 30 s arrow-through-folder stress; session intact; no unrelated process died.
8. **Apartment (L4):** prevhost runs STA, preview works after the `Apartment` change; register → unregister → re-register leaves a clean CLSID tree (C1's parquet half).
9. **Site/visuals (L5):** Tab into/out of the pane; dark-mode background follows; shell UI font (not SYSTEM bitmap) at 150% DPI.
10. **Re-register loop (L6 harness only):** the CoCreateInstance e2e test runs green on this PC if the harness was built.

## local-trading-desk (after H1, M1-M6, L1-L8 fixes)
1. **Export round trip (H1):** with 5,001+ rows, About → export → import on a second, empty database; counts match; UI box has counts.
2. **Inf row (M1):** commit a CSV with `pnl=inf`; preview shows the row as invalid; export to JSON succeeds with nothing unreachable.
3. **Journal view (M2):** commit a CSV; open the stored-journal listing; copy a `jr_…` ID into the Notes tab; link resolves.
4. **Calendar (M3):** simulate an older installed calendar; update the exe (or rerun with a newer bundled file); the installed calendar is replaced with a backup next to it; user-imported file is untouched (config was set) — plus the genuine 2027 data check the Manager chose.
5. **Zone credit (M4):** nothing to run — `grep -i cldr THIRD_PARTY*` is the acceptance.
6. **Risk labels (M5):** USD-vs-JPY example via the UI; reported size matches hand-derived math with the new labels/assumption text.
7. **Broker offset (M6):** run a naive-timestamp CSV with a broker offset configured; correlation-day counts match; assumption line names the zone.
8. **Config recovery (L1):** truncate `config.json` while holding a valid `.bak`; app opens with a warning; no crash.
9. **DPI/manifest (L2):** 150% display: text readable, layout intact; shrink below 900×600 → no black-hole/claustrophobic stale positions; window drag between mixed monitors stays crisp.
10. **UI responsiveness (L3):** commit a ~20 MiB CSV; window stays in "Not Responding" < 2 s; preview shows the cap note.
11. **Keyboard (L4):** Tab through the calculator and Notes fields in order; Enter on focus does the right button.
12. **Map keys (L5):** two devices in a previously-missing zone (e.g. `Ekaterinburg`) show no "unavailable".
13. **Dedupe (L6):** export the same history under `report (1).csv` and re-import → no new rows; preview flags the dupes.
14. **Weekend reopen (L7):** with the custom preset on a Friday after close, "next reopen" shows Monday (not Sat/Sun).
15. **Backup resilience (L8):** run the store's Windows lock test green on this PC; additionally, once through the UI with an AV/watching tool active, backup file intact.
16. **Mutex independence (L9):** with the app running, `go test ./...` passes (no mutex collision).

## mic-mute (after MM-1…MM-9 fixes)
1. **Rebind on audiosrv restart (MM-1):** elevated `net stop audiosrv && net start audiosrv`; press Ctrl+Alt+M once; indicator recovers without device-ID change.
2. **Queue saturation (MM-2):** drag a mic volume slider rapidly for ~10 s with callbacks running; the mute state at the end matches the actual device state (no stuck display).
3. **Consent replay (MM-3):** fresh install (no consent file), press the hotkey twice quickly, click Yes → mic ends **muted**. Also: tray Exit while the dialog is open → app exits without flipping state.
4. **Menu semantics (MM-4):** change the device state externally (Teams unmute/mute) while the tray menu is open, then click the labelled item — the device follows the label, not the pre-menu cache.
5. **Late taskbar (MM-5):** kill Explorer, launch the exe, restart Explorer — icon appears; app never exited ("could not start" absent).
6. **BOM preferences (MM-6):** save preferences via Notepad (UTF-8-BOM), restart app — hotkey loads.
7. **Shutdown bound (MM-7):** with a breakpoint-suspended worker (debugger) simulate the hang; exit trays the icon, window dies, and the process releases the mutex within ~4 s.
8. **DPI/off-screen (MM-8):** two monitors at 100%/150%; drag across (no blur/stretch); save position 1-px overlapping a monitor edge, restart → not off-screen; unplug the monitor it's on → recovered on the remaining screen.
9. **Hotkey lifecycle (MM-9):** before first consent: Ctrl+Alt+M goes to another app's registered handler (or is simply unregistered); after consent → registered; "Disable microphone control" → unregistered immediately.
10. **Read-only warnings (MM-10):** corrupt the preferences store (or launch read-only mode); drag the indicator twice → one warning, not three.

## Housekeeping rules for every run
- Run on this PC only when the finding's own spec marked the check as required; server-class fixes (store, parser) don't need Explorer sessions.
- After the run, append a short "verified on <date>" line to the relevant repo's `docs/TESTING.md` (that file is the landing place each architecture expects).
- Any check that cannot pass → open the finding's spec again; do not hand-wave; report the failure to the Manager.
