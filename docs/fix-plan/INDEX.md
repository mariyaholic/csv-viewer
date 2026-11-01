# Fix-plan INDEX — 07a csv-viewer, 07b parquet-viewer, 08 local-trading-desk, 10 mic-mute

Prepared 2026-11-01 by the fix-planning agent. Specs live in `fix-plan\<repo>\<ID>.md`; the two cross-repo specs live in `fix-plan\xr\`.

## Pinned SHAs (verify before each fix; specs cite file:line at these commits)

| Repo | Dir | Pinned HEAD (all clean, main) | Re-reviewed at |
|---|---|---|---|
| 07a csv-viewer | `repos\csv-viewer` | `eb3580da4c5820b9ca930c26de9b3f53a4839b2b` | Opus review `eb3580d` — identical |
| 07b parquet-viewer | `repos\parquet-viewer` | `e815e646e6d261291417730ac79345da81e7f6f9` | Opus review `e815e64` — identical |
| 08 local-trading-desk | `repos\local-trading-desk` | `49802a4288583bfa978f56a05ce4e980475240d1` | Opus review `49802a4` — identical |
| 10 mic-mute | `repos\mic-mute` | `08396365566ec6c4427dc65a608a93cfb6836536` | Opus review `0839636` — identical |

Every finding was re-verified against the cloned code before its spec was written: **54/54 confirmed or confirmed-with-caveat; 1 partial (08 L5, details in its spec); none already fixed, none unconfirmed.** Stay on `main`; one fresh fixing session per finding, in the Run order below (the row order is the priority the Manager's brief mandates — highs → ⚠ → mediums → lows — and "Depends on" carries the truth about sequencing; exceptions are footnoted).

CI action pins for XR-1 (verified 2026-11-01 by `git ls-remote`): `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0) · `actions/setup-go@40f1582b2485089dde7abd97c1529aa768e1baff` (v5.6.0) · `dtolnay/rust-toolchain@cfcaf53ba4c621917ec7f78a66bff7a8848ddad4` (1.85.0, csv-viewer) · `dtolnay/rust-toolchain@ccc8c1f930153b792217f06c65a38f23a882993a` (1.85.1, parquet-viewer). The dtolnay action bakes the toolchain version into the pinned ref's action.yml, so those SHAs are version-specific.

## Master table

Columns: **Run** (suggested execution sequence), ID, Sev, Confirmed (at pinned SHA), Size, Depends on, Needs Manager.

### Highs

| Run | Repo | ID | Sev | Conf. | Size | Depends on | Needs Manager |
|---|---|---|---|---|---|---|---|
| 1 | 07b parquet-viewer | H1 | high | Yes | S | — | No |
| 2 | 07a csv-viewer | 07A-01 | high | Yes | M | — | Yes (variant choice) |
| 3 | 07a csv-viewer | 07A-02 | high | Yes | L | 07A-01 | Yes (off-thread design) |
| 4 | 08 local-trading-desk | H1 | high | Yes | S | — | No |

### ⚠ sign-off items (run once the Manager's sign-off line exists)

| Run | Repo | ID | Sev | Conf. | Size | Depends on | Needs Manager |
|---|---|---|---|---|---|---|---|
| 5 | 08 local-trading-desk | M3 | medium | Yes | M | — | **Yes (⚠)** + 2027-data decision |
| 6 | 08 local-trading-desk | M4 | medium | Yes | S | — | **Yes (⚠ + C2/CLDR version)** |
| 7 | 08 local-trading-desk | M5 | medium | Yes | S | — | **Yes (⚠ semantics)** |
| 8 | 07b parquet-viewer | M2 | medium | Yes | S | — | **Yes (⚠ sweep approval)** |
| 9 | 10 mic-mute | MM-3 | medium | Yes | S | MM-1 (Run 10) | **Yes (⚠ discard policy)** |

### Mediums

| Run | Repo | ID | Sev | Conf. | Size | Depends on | Needs Manager |
|---|---|---|---|---|---|---|---|
| 10 | 10 mic-mute | MM-1 | medium | Yes | S | — | No |
| 11 | 10 mic-mute | MM-2 | medium | Yes | M | — | No |
| 12 | 08 local-trading-desk | M1 | medium | Yes | S | — | No |
| 13 | 08 local-trading-desk | M6 | medium | Yes | M | M1 (same file batch) | No |
| 14 | 08 local-trading-desk | L3† | low | Yes | S | — | No |
| 15 | 08 local-trading-desk | M2 | medium | Yes | M | L3 (same preview Journal path) | No |
| 16 | 07b parquet-viewer | M3 | medium | Yes | M | C2 ruling | Yes (C2) |
| 17 | 07b parquet-viewer | M1 | medium | Yes | L/S(fallback) | H1 (Run 1) | Yes (fix-vs-disclose) |
| 18 | 07a csv-viewer | 07A-03 | medium | Yes | S | — | No |
| 19 | 07a csv-viewer | 07A-04 | medium | Yes | M | — | No (budget x4 default) |
| 20 | 07a csv-viewer | 07A-05 | medium | Yes | M | before 07A-02 ideally | No |
| 21 | 07a csv-viewer | 07A-06 | medium | Yes | M | 07A-05 (same spawns) | No |
| 22 | 07a csv-viewer | 07A-07 | medium | Yes | M | 07A-01 (reg block), 07A-08 | No |

### Lows

| Run | Repo | ID | Sev | Conf. | Size | Depends on | Needs Manager |
|---|---|---|---|---|---|---|---|
| 23 | 07b parquet-viewer | L6 | low | Yes | M | — | No (CI-minutes Q) |
| 24 | 07b parquet-viewer | L5 | low | Yes | M | L6 harness first | No |
| 25 | 07b parquet-viewer | L3 | low | Yes | S | — (before L5, same file) | No |
| 26 | 07b parquet-viewer | L4 | low | Yes | S | — | No |
| 27 | 07b parquet-viewer | L7 | low | Yes | S/M | — | No (docs vs watchdog) |
| 28 | 07b parquet-viewer | L8 | low | Yes | S | XR-1 | No |
| 29 | 07b parquet-viewer | L1 | low | Yes | S | C1 ruling | Yes (C1) |
| 30 | 07b parquet-viewer | L2 | low | Yes | S | L1 (same fn) | Yes (C1) |
| 31 | 10 mic-mute | MM-4 | low | Yes | M | MM-1 | No |
| 32 | 10 mic-mute | MM-9 | low | Yes | M | MM-3 (consent flow) | No (UX confirm) |
| 33 | 10 mic-mute | MM-5 | low | Yes | S | — | No (silent-icon Q) |
| 34 | 10 mic-mute | MM-10 | low | Yes | S | — | No |
| 35 | 10 mic-mute | MM-6 | low | Yes | S | — | No |
| 36 | 10 mic-mute | MM-7 | low | Yes | S | — | No (3 s confirm) |
| 37 | 10 mic-mute | MM-8 | low | Yes | M | — | No (PerMonitorV2 confirm) |
| 38 | 10 mic-mute | MM-11 | low | Yes | M | MM-2, MM-3 land first | No |
| 39 | 10 mic-mute | MM-12 | low | Yes | M | XR-1 | **Yes (installer/uninstaller)** |
| 40 | 08 local-trading-desk | L1 | low | Yes | S | — | No |
| 41 | 08 local-trading-desk | L7 | low | Yes | S | — | No |
| 42 | 08 local-trading-desk | L6 | low | Yes | S | — | No |
| 43 | 08 local-trading-desk | L8 | low | Yes | S | — | No |
| 44 | 08 local-trading-desk | L9 | low | Yes | S | run last (tests ride H1/M1/L5) | No |
| 45 | 08 local-trading-desk | L5 | low | **Partial** | M | M4 (CLDR version) | No (but uses M4's decisions) |
| 46 | 08 local-trading-desk | L4 | low | Yes | S | — (before L2) | No |
| 47 | 08 local-trading-desk | L2 | low | Yes | L/S | M2/L3/L4 (ui cluster) | Yes (S-vs-L scope) |
| 48 | 07a csv-viewer | 07A-08 | low | Yes | S | C1 ruling, 07A-07 | **Yes (C1)** |
| 49 | 07a csv-viewer | 07A-09 | low | Yes | S | 07A-02 (same function) | No |
| 50 | 07a csv-viewer | 07A-10 | low | Yes | S | — | No |
| 51 | 07a csv-viewer | 07A-11 | low | Yes | M/S | — | No |
| 52 | 07a csv-viewer | 07A-12 | low | Yes | S | 07A-02 extraction | No |
| 53 | 07a csv-viewer | 07A-13 | low | Yes | S | XR-1 | Yes (`-D warnings` policy) |
| 54 | 07a csv-viewer | 07A-14 | low | Yes | S | C2 ruling | Yes (C2) |
| — | cross-repo | XR-1 | low | Yes (all 4) | S×4 | per repo, ideally first | **Yes (CI minutes + agent-file policy)** |
| — | cross-repo | XR-2 | manual | N/A | M | each repo's block last | No |

† 08 L3 is a low that runs *before* 08 M2 (same `previewJournal` path; the cap lands first so M2 decorates a bounded loop). Noted here under mediums purely as an execution note.

## Pending Manager decisions

**All resolved on 2026-11-01** — see `fix-plan\DECISIONS.md` (the working-approval record: C1/C2 rulings, the five ⚠ sign-off lines verbatim, scope choices, data plan, sequencing). Fixing sessions read DECISIONS.md first. The table below is kept as the historical record of what blocked planning.

| Decision | Blocks | Where asked |
|---|---|---|
| C1 — shared uninstall policy (CLSID tree + empty leaf) | 07A-08, 07b L1, 07b L2 | 07A-08.md, 07b L1/L2 |
| C2 — one NOTICE rule (template-first default proposed) | 07A-14, 07b M3, 08 M4 | 07A-14.md, 07b M3.md, 08 M4.md |
| ⚠ sign-off lines (4 + 1) | 07b M2, 08 M3/M4/M5, 10 MM-3 | each spec's header |
| 07A-01: `DisableLowILProcessIsolation` vs LocalLow spool | 07A-01, then 07A-02 | 07A-01.md |
| 07A-02: chunked-STA vs stream marshalling | 07A-02 | 07A-02.md |
| 07b M1: fix (marshalling, L) vs disclose (README, S) | 07b M1 | 07b M1.md |
| 08 M3: 2027 calendar data source + release timing | 08 M3 data half | 08 M3.md |
| 08 M2/L2: journal-view scope; DPI S-vs-L | 08 M2, 08 L2 | 08 M2.md / L2.md |
| 10 MM-12: portable exe vs uninstaller (SignPath) | repo 10 packaging | 10 MM-12.md |
| XR-1: CI minutes on private repos; agent-file removal policy | XR-1 | xr/XR-1.md |

## Notes for the fixing agent

- One finding per session; keep each commit scoped to its spec. Specs are pinned to the SHAs above — if a repo has moved, re-verify the cited lines before coding and note the drift in the session.
- Clusters that want sequential sessions in one repo (same file): mic-mute MM-1→MM-3→MM-9 and MM-2→MM-11; parquet H1→(L6 harness)→L5→M1, L1→L2; csv-viewer 07A-01→07A-07→07A-08 (registry block), 07A-05→07A-06→07A-02, 07A-02+07A-09+07A-12; trading-desk H1+M1 (store/journal), M2+L3+L4+L2 (ui).
- Windows-behaviour checks this PC can run: every spec marks its manual steps; XR-2 collects them per repo.
