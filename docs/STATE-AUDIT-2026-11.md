# State-machine review follow-up

Base: `94b69e35baab64e018eb865443159b5355f2af12`. Existing app themes and assets are preserved; no shelved theme changes are included. The existing STATE-MACHINES inventory describes scope, not proof of every possible interleaving.

## Corrections

Worker cancellation now holds the current-ID guard through the flag update, with the same cancelled/current lock order as begin. Active cancellation belongs to the active request and is not retained for a later reused ID. A different pre-cancelled ID is still consumed once. The regression distinguishes those states.

Supervisor cancellation remains nonblocking. Two process tests now wait for the background child owner and operation-slot cleanup before releasing their shared test mutex, preventing a later test from observing unfinished cleanup. Production supervisor behavior is unchanged. The same test correction is mirrored to the sibling Parquet repository.

## Verification

The initial audit reproduced the cancellation race on the baseline, then passed 190 Linux tests, one million cancellation transitions and 15 repeated supervisor suites. Integration reconstructed and verified every tracked blob from pinned main, including binary assets and fixtures, then applied and reviewed the focused patch. Current full workspace/Windows/native smoke results belong to the exact PR head's CI, not historical runs.

Formatting, offline documentation generation, six documentation tests, 18 design tests and original theme-token parity are checked. Native Windows CI includes strict Clippy, full workspace tests and standalone control/paging smoke. Wine aborts during prefix initialization before the application runs; no Wine runtime pass is claimed. Live Explorer/COM/UI acceptance beyond the existing native harness remains a limit.

## Protocol limits

Distinct outstanding requests must not reuse an ID: cancellation messages cannot distinguish two simultaneous requests with the same ID. This correction removes active-cancel retention and stale-ID cross-cancellation; it does not redesign request identity or prove all scheduling interleavings. No merge, release or deployment is performed.
