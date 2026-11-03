# Large-file browsing

## Controls and search scope

- Previous/Next read bounded pages, not the original 200-row sample. Up to 200
  rows and 64 source columns are retained per page; text limits can reduce that.
- Ctrl+F focuses the search field; Enter or Find starts at the first source row.
  Matching is literal Unicode-lowercase substring matching, not regex, sorting,
  SQL or an index of every cell. `needle.*` means those literal characters.
- Next continues from the last source cursor, including a search slice with no
  matches. No matches in an unfinished slice does **not** mean no file-wide matches.
  Status reports scanned rows and whether continuation remains available.
- Clear the field and Find to return to normal browsing. Previous revisits the
  last successful cursor; at most 256 previous cursors are retained. Cancel/error
  does not commit navigation. The inspector identifies the selected source row.
- F5 restarts at row one with the current applied query and adopts fresh source
  metadata. Changing files clears the query. Failed requests preserve the old page.
- CSV source row numbers count logical data records after the automatically
  detected header, not physical lines. Multiline fields remain one record.

## Format-specific limits

CSV accepts regular files up to **4 GiB**, with **16 KiB decoded fields**, **1 MiB
records**, **1 MiB retained raw text**, and a **20-second parse budget**. It preserves
leading zeros and uses the existing encoding/dialect/header inference. UTF-8
pages resume at record-boundary byte checkpoints bound to source length/write-time;
CRLF, BOMs and quoted multiline values are handled without replaying the prefix.
UTF-16 and Windows-1252 replay the decoded prefix instead: decoded byte positions
are never mistaken for source positions. Later pages in those encodings can hit
the time limit before reaching the requested offset and report an error rather
than claiming EOF. UTF-8 decoding errors and malformed/oversized records are errors.
Search checks decoded text in the first 64 columns, including text beyond a cell's
shorter display limit; other columns are not searched. These are browsing windows,
not full-profile reports. CSV overrides remain in the existing CLI, not this UI.

Parquet accepts files within the existing core's **16 GiB** source cap. Each helper
seeks the file footer and relevant row groups; it does not transfer the entire
file into a worker spool. Search operates on bounded typed/rendered text in the
first 64 columns, not unrendered nested/binary data or truncated string tails.
Each request scans at most **20,000 source rows** or approximately **20 seconds**
plus the currently bounded decode call, retaining at most **4 MiB** before display
bounds. Next continues a limited scan. Core footer/row-group/decode limits still
apply; a decode error is not treated as a successful end-of-file search.
Metadata-reported row counts and exhaustion remain explicitly unverified.

Shared display limits remain 4 KiB/cell, 8 MiB retained display text and 128 KiB
metadata. The owner-data table contains only the current page, not the entire file.
CSV can offer one final empty continuation when the last match exactly fills a
page; Next is continuation availability, not a promised nonempty result page.

## Isolation and compatibility

The GUI launches **the same desktop executable in explicit `--browse-worker`
mode** for each page/search operation. Only that child opens/parses the source;
blocking I/O and framed result decoding run off the HWND thread. Requests and
responses are length-prefixed JSON capped at 16 MiB. Source length/write-time is
checked before/after reading and carried between pages; this detects ordinary
changes, not adversarial same-metadata edits or a cryptographic snapshot.

The existing owned-child lifecycle, Windows kill-on-close job, cancellation,
120-second independent watchdog, stale-result generation checks and operation
limits remain in place. No full-source copy is made for GUI browsing. It is not a
new low-integrity sandbox: the helper has the user's normal read permissions.
Explorer/COM workers, wire protocols, Low-IL/stream behavior and CLI registration
remain unchanged. The legacy desktop `--check` path still exercises the original
64 MiB inspection adapter and its matched companion worker. No defaults,
associations, theme preferences or window settings are written by this feature.

## Verification and remaining acceptance

Local full-workspace Linux and Windows-GNU/Wine suites pass, including the
explicit CSV worker integration case and opt-in Parquet COM/200 MiB E2E coverage.
Real native control smoke exercises Next/Previous, exact source row numbers,
literal search at source row 451, no-match exhaustion, clearing, cancellation,
reload, copy and all six themes. Both GUIs also opened generated 80,000-row files
larger than 64 MiB (CSV 83,920,016 bytes; Parquet 84,274,434 bytes).

CSV tests cover UTF-8 checkpoints with CRLF/multiline/BOMs, a real leading BOM
value in a later record, malformed quote/record bounds, legacy encodings and a
late UTF-8 page that reads less than 70 KiB rather than replaying its prefix.
Shared tests cover navigation bounds, stale results, blocked-worker cancellation,
watchdog behavior and malformed/oversized transport frames. CI additionally runs
the actual MSVC native paging/search smoke on deterministic 600-row fixtures.

Wine evidence is not normal Windows desktop acceptance. Maria's existing local
checklist for Explorer/default apps, Narrator/UIA, DPI/monitor/high-contrast and
physical interaction remains pending. Source caps are limits, not a claim that
4 GiB/16 GiB inputs, every encoding, or adversarial files were exhaustively tested.
