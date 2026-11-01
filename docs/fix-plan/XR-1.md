# XR-1 — Cross-repo: first CI run, SHA-pinned actions, and removing the tracked AGENTS.md / opencode.json

- Applies to: all four repos — csv-viewer, parquet-viewer, local-trading-desk, mic-mute (each pinned SHA at the bottom of INDEX.md).
- Severity: low per repo, cross-cutting (release-readiness gate)
- Status: **CONFIRMED in all four** — each repo has exactly one workflow (`.github/workflows/ci.yml`), every action reference carries `# TODO pin to commit SHA before publishing`, no repo has a release workflow or cargo-deny/go-licenses step, and `git ls-files` shows **AGENTS.md and opencode.json are tracked in all four**.
  - csv-viewer `ci.yml:9-10`: `actions/checkout@v4`, `dtolnay/rust-toolchain@1.85.0` (`with: components: rustfmt, clippy`)
  - parquet-viewer `ci.yml:9-10`: `actions/checkout@v4`, `dtolnay/rust-toolchain@1.85.1` (same components). `deny.toml` exists but CI never runs it.
  - local-trading-desk `ci.yml:9-10`: `actions/checkout@v4`, `actions/setup-go@v5` (`go-version-file: go.mod`); steps gofmt → vet → build → test (pwsh gofmt check)
  - mic-mute `ci.yml:9-10`: `actions/checkout@v4`, `actions/setup-go@v5` (`go-version: '1.24.x'`); same shape
- Per-repo-specifics that overlap: 07A-13 (clippy without `-D warnings`, no deny/release), 07b L8 (no deny/release job), MM-12 (no release workflow), 08's CI note. The findings' repo specs own those repo-specific items; this spec owns the three shared actions.
- Size: S per repo · Model effort: entry-level
- Manager sign-off: **Yes — one policy line**: "CI may run on the private repos (billable minutes), and the internal agent workspace files stay local-only everywhere."
- This is not a finding by a review ID; track it in INDEX as XR-1.

## Root cause
The repos were written offline; automation has never executed, so the workflow TODOs were never rotated into pins, and the agent-workspace files (which contain internal paths and should not be public) were committed at scaffold time and never removed from git tracking (working-tree copies are legal to keep local-only).

## Fix (identical recipe ×4)
1. **Pin by commit SHA** in each `ci.yml` (keep a `# vX.Y.Z` comment):
   - Both repos' checkout: `uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0` (SHA verified 2026-11-01 via `git ls-remote`).
   - Go repos (08, 10): `uses: actions/setup-go@40f1582b2485089dde7abd97c1529aa768e1baff # v5.6.0`.
   - csv-viewer (Rust 1.85.0): `uses: dtolnay/rust-toolchain@cfcaf53ba4c621917ec7f78a66bff7a8848ddad4 # 1.85.0` (SHA verified 2026-11-01; note this action's version selection is *baked into the ref's action.yml*, so these SHAs are version-specific — comment with the version so bumps are deliberate).
   - parquet-viewer (Rust 1.85.1): `uses: dtolnay/rust-toolchain@ccc8c1f930153b792217f06c65a38f23a882993a # 1.85.1`.
2. **Remove agent files from git, keep local copies**: `git rm --cached AGENTS.md opencode.json` (+ add both to `.gitignore`), commit "chore: keep agent workspace files local-only". Never `rm` the working-tree copies — the agent harness still reads them.
3. **Run CI once on `main`** for each repo (AFTER steps 1-2, so the first run is the pinned one). All four repos are private → minutes are billable (09's experience, VERIFY_09-10 note). Windows-latest runners cover 07a/07b (Rust GNU build per review note — cargo in CI runs on `windows-latest` after fix; the 07a script builds GNU but CI's own build is fine) and run the Go/coordination checks.
- Rejected alternative: pin by branch/tag ref (`@v4`). Updateable Pin-notary story, not release audit; the TODO comments themselves say "commit SHA".

## Acceptance test
- Automated: after pinning, the workflow run log shows the SHA-suffix in "Get PR at …" step names (or actions/…@sha in annotations); a repo-side static grep asserting no `uses:` line lacks a 40-hex SHA is a trivially checkable gate.
- Fails-before: the grep gate finds `@v4`/`@v5`/`@1.85.x` → red; after: green.
- CI green: after the fix batch runs, `gh run list --limit 1 -R mariyaholic/…` per repo shows the first successful pinned run (zero runs assumed acceptible to replace once — this is the first run).

## Risk / blast radius
- Workflow-file-only + a `.gitignore`. No application code. The release workflow/toolchain items belong to the per-repo specs (07A-13, 07b L8, MM-12) — this spec must not block on them.
- If CI reveals a real failure on first run, that's expected on a fresh pipeline (windows-latest gofmt check, vet) — the fixer reports it; it is not a scope creep to fix the workflow syntax.

## Open questions for the Manager
1. Confirm consumption pattern: run CI immediately after the fix batch per repo, or batch-order per the INDEX's suggested order? (Suggested: per repo, immediately after that repo's code fixes.)
2. Confirm: agent-file removal stays local-only everywhere (no archiving of the internal).
