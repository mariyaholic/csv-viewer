# Status

Made by Maria. Apache-2.0.

The CSV viewer, parser, bounded preview session, worker, protocol and Windows
Explorer handler are built. Unregistration removes matching owned values and
leaves shared registry keys in place; empty keys are expected residue. The
exclusive CLSID tree could be deleted, but is also retained. Rollback restores
prior values when its ownership check succeeds and does not delete keys.

Offline verification with Cargo bin on PATH, `CARGO_NET_OFFLINE=true`, and
`CARGO_TARGET_DIR=/tmp/terra-review/r3/07a-csv-viewer/target`:

- `cargo fmt --check`: passed.
- `cargo test --offline`: 118 passed, 0 failed.
- `cargo build --offline`: passed.
- `cargo clippy --offline --all-targets`: passed, 4 warning lines.
- `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu`: passed, 32 warning lines.
- `cargo build --offline --target x86_64-pc-windows-gnu`: passed.
- `cargo test --offline --target x86_64-pc-windows-gnu --no-run`: passed; binaries not executed.
- `cargo deny --offline check licenses`: passed; three unused-allowance warnings.

Rust source files total 10,383 lines including tests (`rg --files -g '*.rs' crates | xargs wc -l`).
Real Windows checks and open issues are listed in docs/TESTING.md: synchronous
stream copying can delay the host; registry value ownership checks and rollback
can race concurrent changes. Visual layout, stream apartment/lifetime, registry
behavior, packaging and deployment remain unverified on Windows. Clippy retains
FFI naming and enum-size warnings.
