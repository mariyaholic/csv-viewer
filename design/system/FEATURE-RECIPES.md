# Feature recipes and evolution policy

These are planned interaction recipes, not implemented capabilities. Reuse the
same components and semantic roles in both apps; format adapters decide which
capabilities are available. Unavailable capabilities are omitted or disabled with
a reachable explanation, never replaced with fake data.

## Find, filter and sort

**Surface:** labelled search row above the grid; compact filter chips below it.
**Scope:** shown sample first. The match count includes the searched population.
**Cost:** in-memory sample operations may be immediate; global operations require
worker capabilities, bounded result transport and separate progress/cancellation.
**Recovery:** clearing filters restores selection by stable record identity when
available, not by the old row number. Otherwise clear selection and announce it.
**Gate:** UTF-8/case policy, null vs empty, explicit numeric vs lexical sort,
formula text, no results, incomplete sample, stale results during refresh.
Do not enable global CSV search without a record-aware index or explicit scan.

## Paging and large-file navigation

**Surface:** next/previous page, page-size control and explicit scope beside the
coverage strip. Keyboard commands must not collide with grid navigation.
**Scope:** loaded page vs complete source. Do not invent a last-page count from
unverified metadata or byte offsets. Unknown total gets **More**, not `page 1 of ?`.
**Cost:** CSV needs record-aware indexing across quoted multiline fields; Parquet
needs row-group/page worker access. The current preview protocol does not provide
these merely because the shell has a grid.
**Recovery:** preserve current page while the next loads, mark it previous/stale,
ignore late generations and keep Cancel available. No prefetch beyond a declared
memory/I/O budget. Gates include changed source, truncated tail and index mismatch.

## Schema, column visibility and structured cells

**Surface:** inspector tabs for Value / Schema / Diagnostics; optional column menu.
**Scope:** distinguish physical type, logical type and inferred display type.
**Cost:** bounded tree depth/node count, cancellation and explicit omitted nodes.
**Recovery:** changing visible columns never reinterprets source data. A hidden
column remains in the model only if its retention budget allows; explain reloading.
**Gate:** nested lists/maps, huge values, nullability, duplicate column names,
non-ASCII names and a selected column disappearing after reload.

## Full scans, validation and profiling

**Surface:** a named command and persistent progress strip, not automatic on open.
**Scope:** label metadata-derived estimates, sampled observations and verified
scan facts independently. A profile panel must not imply uniqueness from a sample.
**Cost:** consent before an expensive read; disclose source size and known limits.
Enforce worker timeout, job ownership and bounded summary output.
**Recovery:** cancelled/partial profiles stay partial; old complete results become
stale after source mutation. Retry is explicit, not an invisible polling loop.
**Gate:** no percentage without a denominator, changes mid-scan, malformed late
records, worker crash, full disk, deadline and source access revoked.

## Export and copy

**Surface:** scope-specific menu entries: **Copy shown row**, **Export filtered
sample**, **Export page**, or **Export complete result** only when actually supported.
**Scope:** always name whether original values or display strings are written.
**Cost:** native Save dialog, no overwrite without confirmation, write temp then
atomic replace when supported; validate source and destination are not the same.
**Recovery:** cancellation cannot leave a partial file presented as complete.
**Gate:** CSV quoting, multiline values, encoding, formula-injection policy,
null semantics, numeric precision and destination permissions. Copying untrusted
text is not the same as making it safe for a spreadsheet to evaluate.

## Multi-file comparison

**Surface:** later milestone; two clearly named documents and a diff legend.
**Scope:** explicit join/key policy, duplicate keys and unmatched records. Compare
samples only under a conspicuous **sample comparison** label.
**Cost:** two documents require revised per-process memory and worker budgets;
never multiply the current limits silently. Sharing one shell is not sharing one
mutable document controller.
**Recovery:** independent load/error/selection state and coordinated cancellation.
**Gate:** no red/green-only differences; include Added/Removed/Changed text,
null/empty distinctions and an explicit unknown/not-compared state.

## Preferences and extension seams

- Theme and density are independent. Type scale and wrapping are user preferences;
  semantic numeric formatting is a data policy, not a brand effect.
- A format adapter exposes capabilities: sample filtering, source paging, full
  scan, raw-value copy, structured-value view and export. The shell renders these
  consistently and never infers support from the file extension alone.
- Retain operation generation, source fingerprint and coverage provenance in the
  view model. A new panel consumes that model rather than starting its own parser.
- Add a semantic token only when an existing role cannot describe the purpose.
  Add it for all six themes, document final foreground/background pairs, bind it
  once per component and add a validator test. No `dolorisButton` component fork.
- Contract versioning: patch for clarified docs without changed meaning; minor
  for additive roles/components; major for renamed roles, changed scope semantics
  or incompatible interaction behavior. Ship migrations and before/after examples.
- A token update is not automatically a native app update. Runtime adoption needs
  generated mapping/parity checks and the native component acceptance matrix.

## Pull-request checklist

1. What user question does this feature answer, on what data scope?
2. Which shared components and existing roles does it reuse?
3. What does each of the seven operation states look/sound like?
4. What is the resource budget; who owns cancellation and stale results?
5. What is keyboard focus before, during and after the operation?
6. What changes for high contrast, text scaling and absent fonts?
7. Does the validator pass, and do contract snapshots match both viewers?
8. Are screenshots labelled real runtime vs illustrative future design?
9. Which native acceptance evidence is attached, and what remains unverified?
