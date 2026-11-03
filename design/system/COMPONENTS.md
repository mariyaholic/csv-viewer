# Component contracts

Shared anatomy and behavior for all six faces. Everything marked **future** is a
specification, not a shipped feature. Bind colors through `tokens.json.components`;
the native adapter may retain OS buttons, selection, menus and forced colors.

## 1. Command bar

Order: **file → data operation → view → overflow**. Open/Reload/Cancel keep their
current meanings. One prominent action per context; destructive commands never
borrow the brand action appearance. Move low-frequency commands into a native
menu before the bar wraps. Search gets a dedicated second row when needed.

Default/hover/pressed/focused/disabled/busy states use the same geometry. A focus
outline is 2 DIP with 2 DIP breathing room; don't place a gold ring directly on a
gold button where it vanishes. Disabled controls retain explanatory text; do not
reduce the opacity of the entire row. A disabled reason is reachable by keyboard
and assistive technology, not a hover-only tooltip. Busy is not disabled: Cancel
remains actionable and is announced as cancelling until acknowledgement.

Keep Ctrl+O, F5, Escape and existing table-focused Ctrl+C. Reserve Ctrl+F for a
future find field. Do not promise shortcuts for unimplemented commands. Standard
native menus expose shortcuts in their item text; avoid a competing command palette
until the command inventory warrants it.

## 2. Inputs, search and filters — future

A field has a persistent label, current value, scope/helper text, clear action
when useful, and an error slot. Placeholder is an example, never the only label.
Default/hover/focus/invalid/disabled/busy are explicit. Errors use both text and a
status marker; invalid does not destroy the typed value.

Find text and structured filters are separate operations. A filter chip includes
field, operator and value; its remove action has a complete accessible name.
Search always says **shown sample**, **loaded page** or **whole file**. A worker
scan cannot inherit the appearance of instant in-memory filtering. Do not announce
"no matches" as a dataset-wide fact when only the sample was searched.

## 3. Data grid

Column header, optional type marker, data cells, row selection and scope footer.
Virtualization never implies full-file access. Table keyboard navigation follows
the chosen native control; expose row/column names and values via UIA. Native
selection colors and system focus take priority over authored fills.

- Left-align text, identifiers and timestamps; right-align proven numeric values.
  Preserve raw lexical values such as `001024`; do not silently convert CSV types.
- A visual ellipsis means display truncation. The inspector explains the cap.
- A genuine null marker, empty text, whitespace and literal text `null` are distinct.
  Use explicit labels in the inspector; a dash is not an all-purpose null marker.
- Sorting is **future**. A sort arrow requires a direction and accessible description
  including its scope. Never show a whole-column ordering claim for sampled rows.
- Frozen columns, row numbers and zebra striping are optional future capabilities,
  not ornament every table must acquire. A row number is its displayed position
  unless a stable source record index actually exists.

Hover is a supporting cue. Selected rows have text/focus/outline cues as well as
color. Do not recolor data based on inferred sentiment. Decimal precision is a
formatting policy, not a theme token.

## 4. Inspector and schema navigation

The inspector names its source: **selected row**, **selected cell**, **schema**,
**file metadata**, or **diagnostics**. Never mix metadata row counts with verified
scan results without labels. Maintain independent scrolling and a visible heading.

A future cell view exposes field name, type provenance, displayed value, truncation
reason, and explicit actions: **Copy shown text**, **Copy original value** or
**Copy structured JSON**. Only show actions supported by retained data/protocols.
Parquet quoted display strings must not masquerade as raw values. Multiline content
wraps or scrolls without reformatting the source; escaping and human-readable mode
are separate labelled representations.

A future schema tree uses expandable nodes, optional/required/nullability labels,
physical/logical type detail, keyboard expand/collapse and positional announcements.
A truncated tree has an explicit omitted-nodes item; it does not pretend the schema
ends there. Tabs switch inspector modes, not independent open documents. Use a
native tab control unless the native acceptance bench rejects it.

## 5. Coverage strip — always present

Separate **operation state** from **data coverage**:

- Operation: empty, loading, ready, partial, error, cancelled, stale.
- Coverage: retained sample/page, metadata-reported, scanned subset, verified full
  scan, unavailable. "Ready" alone says nothing about coverage.

Use text such as `200 retained rows · sample only` or `60 metadata-reported rows
(unverified)`. Those counts must come from the result, not a design constant.
A future filter adds `8 matches in 200 shown rows`; it must not replace scope.
A failed reload retains the old preview with **Previous result** and its timestamp
or generation, plus the failed operation's recovery action. New-file failures must
not leave the old file's table looking like the new file.

Never fabricate transfer percentages. Indeterminate stages can say **Transferring**,
**Inspecting**, **Waiting for worker**, **Cancelling**. Keep the previous result
visibly stale while refreshing. Cancellation is not success and not an error.

## 6. Messages and recovery

Inline notice: severity word + concise cause + scope/consequence + one next action.
Examples: `Partial — only the retained sample was searched. Run a full scan.`
The last action is shown only after full scans exist. `Error — worker missing.
Restore the companion executable and reload.` Do not expose private temp paths
or dump unbounded parser output into a dialog.

Warnings, errors and success each have distinct text, not just different colors.
Marigold trim is decorative and cannot stand in for a warning icon. Persistent
errors belong near their affected surface. Toasts are reserved for nonessential
acknowledgements such as copying; failure information must not disappear on a timer.

## 7. Dialogs, menus, splitters and settings — future where absent

Use native file dialogs and menus. A future settings dialog groups Appearance,
Data display and Worker limits separately. Theme and density are independently
previewable; Cancel restores the previous settings. Store only intentional local
preferences, not cell values or sensitive file histories by default.

A resizable inspector divider has a keyboard adjustment path, minimum panel sizes
and a collapse command that restores focus. If width becomes too small for useful
side-by-side data, move the inspector below the grid or offer a keyboard-reachable
inspector view; never shrink text to fit. At short heights, command and coverage
strips remain reachable; vertical content scrolls.

Modal surfaces remember the invoking control, expose name/description, constrain
focus, support Escape where safe and return focus on close. Never nest a settings
modal over an active confirmation. Per-monitor DPI uses logical layout units and
recreates fonts/resources; it must not double-scale existing dimensions.

## Acceptance matrix for every component

Six themes × normal/compact/comfortable density × keyboard-only plus mouse × enabled/disabled/
focused/busy/error. Also native forced colors, reduced motion, 100/150/200% DPI,
text scaling, long labels, CJK, combining marks, RTL data and no bundled fonts.
Read-only input must still be selectable/copyable. OS overrides win; no themed
owner-drawing is accepted merely because a static plate looks good.
