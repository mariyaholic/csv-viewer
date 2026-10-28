# Status

Made by Maria. Apache-2.0.

The CSV viewer, parser, bounded preview session, worker, protocol and Windows
Explorer handler are built. Explorer stream transfer now copies on the COM
caller; registry rollback and client-sized rendering have been added.

Offline verification with Cargo bin on PATH and `CARGO_NET_OFFLINE=true`:

- `cargo fmt --check`: passed.
- `cargo test --offline`: 118 passed, 0 failed.
- `cargo build --offline`: passed.
- `cargo clippy --offline --all-targets`: passed with 4 warning lines.
- `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu`: passed with 32 warning lines.
- `cargo build --offline --target x86_64-pc-windows-gnu`: passed.
- `cargo test --offline --no-run --target x86_64-pc-windows-gnu`: passed; binaries not executed.
- `cargo deny --offline check licenses` and Linux/Windows target-filtered checks: passed.

Rust source files total 10,463 lines (including tests, measured by `wc -l`).
Real Windows checks and open issues are listed in docs/TESTING.md: synchronous
stream copying can delay the host; registry rollback and empty-key deletion can
race concurrent changes, and ownership of created keys is not persisted. Visual
layout, stream apartment/lifetime, registry behavior, packaging and deployment
remain unverified on Windows. Clippy retains FFI naming and enum-size warnings.
