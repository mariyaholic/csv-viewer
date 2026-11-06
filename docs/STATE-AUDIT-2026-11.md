# State-machine review follow-up

Base: `94b69e35baab64e018eb865443159b5355f2af12`. Existing app themes and assets are preserved; no shelved theme changes are included. The existing STATE-MACHINES inventory describes scope, not proof of every possible interleaving.

## Corrections

Worker cancellation now serializes queue registration, begin, cancel and end under one mutex. Only scan requests already received by the reader can retain a pending cancellation. Active cancellation belongs to the current scan; late/unknown IDs are ignored rather than remembered for future reuse. Pending cancellation is consumed by the queued request once. Regression tests cover queued cancellation, completed-ID reuse, result-delivery overlap and concurrent handoffs.

Supervisor cancellation remains nonblocking. Two process tests now wait for the background child owner and operation-slot cleanup before releasing their shared test mutex, preventing a later test from observing unfinished cleanup. Production supervisor behavior is unchanged. The same test correction is mirrored to the sibling Parquet repository.

## Verification

The initial audit reproduced the cancellation race on the baseline, then passed 190 Linux tests, one million cancellation transitions and 15 repeated supervisor suites. Integration reconstructed and verified every tracked blob from pinned main, including binary assets and fixtures, then applied and reviewed the focused patch. Current full workspace/Windows/native smoke results belong to the exact PR head's CI, not historical runs.

Formatting, offline documentation generation, six documentation tests, 18 design tests and original theme-token parity are checked. Native Windows CI includes strict Clippy, full workspace tests and standalone control/paging smoke. Wine aborts during prefix initialization before the application runs; no Wine runtime pass is claimed. Live Explorer/COM/UI acceptance beyond the existing native harness remains a limit.

## Protocol limits

Clients must use distinct IDs for outstanding requests and must not send stale cancellations after reusing an ID: cancellation messages cannot distinguish two simultaneous requests with the same ID. Sequential reuse after completion is supported, and cancels received before the replacement request are ignored; it does not redesign request identity or prove all scheduling interleavings. No merge, release or deployment is performed.


## Review follow-up: completion and settings

The worker ends its active generation before serializing the terminal Result
or Error. A synchronized terminal writer queues and cancels a replacement ID
to prove cancellation cannot be consumed by the old scan during delivery.
Accepted/progress/terminal/error write failures are fatal; partially written
frames are never treated as valid reported request errors.

Windows preview and full-scan entry points use checked settings loading and
show an error before starting work if reading, parsing or validation fails.
Only a missing settings file selects defaults. A Windows-path regression
checks both entry points' common request builder and preservation of bad bytes.


The reader has an eight-message pending queue and uses nonblocking enqueue.
Overflow cancels active work and terminates with a clear protocol error rather
than blocking the reader or allocating an unbounded queue. Pending cancellation
state is bounded by received queued scans, plus at most the rejected enqueue.
The existing per-frame cap still applies; this is not an exact process RSS cap.

Overflow recorded before completion suppresses the scan result and sends a
connection error; closure checks fatal state even when no queued item remains.
Checked settings reject unknown keys and quote-only overrides (an explicit
delimiter is required) rather than silently ignoring user choices. Legacy
settings callers still receive fallback warnings; preview fails closed.
