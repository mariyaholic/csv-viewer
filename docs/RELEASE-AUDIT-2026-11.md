# First-wave preparation audit: 2026-11-07

Baseline: main 44d2ce8b0240d261e551d4f110ea9c54556fb560.
Repository remained private during this work.

## Existing verification

The baseline's [CI run 42197798268](https://github.com/mariyaholic/csv-viewer/actions/runs/42197798268)
passed Linux/Windows builds and tests, strict Windows Clippy, native standalone
control/paging/search smoke and dependency license/bans checks. Its tag-only
release job was skipped. It produced no downloadable Actions artifacts.
The release collection was empty.

This is evidence for that baseline, not a pass for a later preparation commit
or a clean-machine release payload.

## Reachable-history screening

Enumerated all 27 advertised refs (14 branch refs and 13 pull-request head refs).
Their histories contain 91 distinct reachable commits and 407 distinct file
blobs. All recursive trees returned without truncation.

- 396 UTF-8 text blobs were screened for recognizable GitHub/OpenAI/AWS/Slack
  credential and private-key patterns plus personal-email and local-user-path
  patterns. No credible credential pattern was found.
- Three non-UTF-8 CSV fixtures were read from existing source copies after their
  Git blob hashes matched the remote tree. They contain synthetic Ada/André
  examples, not private data.
- Five distinct PNGs were visually inspected. They show theme controls, a
  synthetic pages.csv example and the offline guide, without visible private
  files, accounts or desktop material.
- Three font binaries were inspected for embedded names/copyright/license
  metadata. Cinzel 2.000, JetBrains Mono 2.211 and Outfit 1.100 declare SIL OFL 1.1
  consistently with their adjacent license files.
- Commit author/committer emails were GitHub no-reply or localhost.invalid
  identities. Two historical STATUS.md versions contain a generic build-user
  home path. Removed AGENTS.md and opencode.json remain in history and describe
  development/workspace instructions. Removal from main does not remove history.

This was targeted screening over enumerated reachable content, not a certified
secret scan or exhaustive privacy/legal clearance. It does not cover dangling
objects, unknown/unadvertised refs, remote issue/comment attachments, repository
settings, steganography or every possible credential format. Rerun a maintained
secret scanner over a full fresh mirror and have the owner review public-history
scope immediately before changing visibility. Do not rewrite history silently.

## Distribution finding and preparation

The old tag workflow attached only csv-viewer.exe, csv-worker.exe, csv_preview.dll
and notices. It omitted the standalone desktop, offline Help and font assets,
and could publish before the other jobs succeeded.

The preparation adds an MSVC standalone builder, an exact ZIP allowlist,
per-file and outer SHA256 checksums, source/profile metadata and extraction
regressions. Tags stage review-only artifacts after validation; they no longer
publish a GitHub Release. The project LICENSE and NOTICE remain unchanged.
GNU runtime redistribution is not part of the proposed MSVC beta.

The committed dependency notice bundle already covers the Cargo graph and OFL
texts accompany fonts. Its generator must be rerun in check mode against fetched
locked packages. A fresh vulnerability-database audit and exact release-runtime
inspection were not completed by this screening.

## Verification limits

Cloud workspace and temporary storage were full during preparation. No cache or
user data was deleted. Python ZIP logic was tested in memory with synthetic PE
headers; these are packaging regressions, not real executable acceptance.

The preparation PR adds Windows extracted-debug-ZIP smoke to the existing native
test host. Consult that PR's exact-head checks for its result. The MSVC release
candidate build and clean-machine/physical-input/accessibility checks remain
open until separately recorded in the [release checklist](RELEASE-CHECKLIST.md).
