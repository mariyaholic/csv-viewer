# DECISIONS — working rulings for the 07a/07b/08/10 fix batch

Recorded 2026-11-01. Status: the Manager instructed "do that md then work with what you recommended" — the items below are the planning agent's recommendations, **adopted as working rulings**. Fixing sessions treat this file as the approved source; any line can be vetoed by the Manager by editing it or saying so, and the reading order is: DECISIONS.md first, then the per-finding spec.

Standing puzzle-solved marks: ✅ decided · ⚠ sign-off text granted · 🕙 parked (post-0.1.0 or on branch).

---

## ✅ C1 — uninstall policy (07A-08, 07b L1, 07b L2)

**Decision:** both repos adopt parquet-viewer's policy — unregister deletes the full `CLSID\{…}` tree; csv-viewer additionally deletes the empty `HKCU\Software\Classes\.csv\ShellEx\{8895…}` leaf on uninstall.
**Rejected:** keeping 07a's restore-what-we-wrote policy — leaves the HKCR-masking question alive indefinitely and two mental models.
**Effects:** unblocks INDEX Runs 29/30 (07b L1/L2) and Run 48 (07A-08, leaf-delete path). `delete_empty_key` stays removed by 07b L6 regardless (dead API).
**Follow-up (manual):** XR-2 csv-viewer item 8 — the real-Explorer test of whether an empty HKCU leaf masks an HKLM handler. Record result in `docs/TESTING.md`.

## ✅ C2 — NOTICE rule (07A-14, 07b M3, 08 M4)

**Decision:** one rule for all repos — **template first, then an optional third-party section.**
- NOTICE begins with the exact 4-line template ("Made by Maria. This product includes software developed by Maria.").
- Below it, an optional clearly-titled section: 07b keeps its Apache Arrow/Thrift block; 07a keeps its THIRD_PARTY pointer lines; 08 keeps its pointer lines.
- Generated licence texts live in `THIRD_PARTY_LICENSES.txt` (cargo-about generated, shipped by both build scripts, in SHA256SUMS — 07b M3). The CLDR credit and Unicode License v3 text live in THIRD_PARTY.md / THIRD_PARTY_LICENSES.md (08 M4).
- Doc fallout: `docs/ARCHITECTURE.md` "root NOTICE is exactly the template" wording is amended to the new rule in 07a (07A-14) and 08 (adjacent doc touch, no code); AGENTS.md's "keep NOTICE unchanged" line stays estate-owned — the Manager updates it in her own time.
**Effects:** unblocks Runs 16 (07b M3), 37 (07A-14, Path B), 5-ish (08 M4 credit placement).

## ⚠ Sign-off lines — granted (working approval 2026-11-01)

| Finding | Sign-off line (use verbatim in the fixing session) |
|---|---|
| 07b M2 | "Approved: spool files use delete-on-close and the worker may remove `parquet-preview-*.tmp` files older than 1 hour in the user's temp directory." |
| 08 M3 | "Approved: the app may replace an installed bundled-sourced calendar with a newer bundled copy (backing it up), and will not touch user-imported files." |
| 08 M4 | "Approved: credit Unicode CLDR windowsZones derivation in THIRD_PARTY and bundle the Unicode License v3 text." |
| 08 M5 | "Approved: the canonical semantics are PointValuePerLot in quote currency × Quote→Account rate, labelled as such." |
| 10 MM-3 | "Approved: consent flow may discard duplicate toggle/enable events queued while the consent prompt is open, and Exit during consent completes without changing mute state." |

## ✅ Scope choices

| Item | Decision | Rejected / parked |
|---|---|---|
| 07A-01 | Variant 1: `DisableLowILProcessIsolation=1` (DWORD writer + ownership-checkable removal) | 🕙 Variant 2 (LocalLow spool) parked on branch `variant/07a-01-locallow-spool` (worktree `worktrees\07a-01-alt`) |
| 07A-02 | Stream marshalling to the worker thread (`CoMarshalInterThreadInterfaceInStream`/GIT), shared design with 07b M1 | 🕙 Chunked-STA fallback parked on branch `variant/07a-02-chunked-sta` (worktree `worktrees\07a-02-sta`) |
| 07b M1 | **Fix** (marshalling, design as 07A-02), not README disclosure | Disclosure only if the fixer reports the COM work blocked late in the cycle |
| 08 M2 | Last-500 stored-journal listing, cap stated in the pane header; IDs shown in preview rows | Full paged viewer = v2 |
| 08 L2 | S now: manifest + DPI-scaled font/layout + clamp | 🕙 L (full per-monitor refactor) post-0.1.0 |
| 07A-13 | Triage the 32 warnings into commented `#[allow]`s, then `-D warnings` in clippy step | Leaving clippy non-strict rejected |
| 10 MM-9 | Hotkey registers only while control is enabled (bound at consent, unbound on Disable) | Documenting the pre-consent grab rejected |
| 10 MM-12 | Portable signed exe for 0.1.0 + uninstall doc ("delete the exe; the app registers no uninstall keys") | A minimal uninstaller script is built only if SignPath says the portable story is insufficient — the Manager asks SignPath |
| 07b L6 | The Windows-CI CoCreateInstance e2e harness is approved (extra Windows-latest minutes accepted) | — |
| 08 M4 | Credit added now; `MappingVersion` stays stable and is updated only when 08 L5 regenerates the table (the CLDR version is recorded then) | Renaming `MappingVersion` during M4 rejected — churn without a verified version |

## ✅ Data / provenance

- **08 M3 calendar:** decouple data from release. Land the upgrade-path fix (Runs 5) now; ship 0.1.0 with the 2026 data + working upgrade path; produce `calendar-2027.json` via the SOURCES.md method with the **Manager verifying each `verified_at`** before it's bundled (data task stays with her; not a fixer task).
- **08 M4/L5 CLDR:** the fixer regenerates the zone table (`08 L5`) from the current published `windowsZones.xml` at fix time, reads the version from the file itself, and records that version in `MappingVersion` + THIRD_PARTY. No hand-patched keys.
- **CI (XR-1):** approved — per-repo CI run immediately after each repo's code batch, on the pinned workflow. No per-push runs on WIP branches; the mic-mute device job (10 MM-11) stays env-gated/opt-in.

## ✅ Sequencing

- Repo order: **10 mic-mute → 08 local-trading-desk → 07b parquet-viewer → 07a csv-viewer** (fewest blockers first, per SUMMARY).
- Within a repo: follow the Run column of INDEX.md; the cross-cluster session notes in INDEX "Notes for the fixing agent" hold.
- XR-1 lands per-repo **before** that repo's last fix (so the first pinned CI run covers everything); XR-2 runs **after** it.
- Specs stay pinned to the SHAs in INDEX.md; if a repo has moved when a fixing session opens, re-verify the cited lines first and note drift.

## Worktrees / branches created (scaffolding only, no code written)

| Worktree | Branch | Purpose |
|---|---|---|
| `worktrees\07a-01-alt` | `variant/07a-01-locallow-spool` | Houses the parked LocalLow-spool variant of 07A-01 if it is ever picked up |
| `worktrees\07a-02-sta` | `variant/07a-02-chunked-sta` | Houses the chunked-STA fallback of 07A-02 if marshalling turns out blocked |

Branches sit on the existing clones (`repos\csv-viewer`); worktrees are outside `repos\` (planning-phase hands-off respected). Deleting them later: `git worktree remove` + `git branch -D` — safe, they start at the pinned SHAs with no commits.
