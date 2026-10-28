# Status

The workspace contains a read-only CSV parser, terminal viewer, worker and
Windows Explorer preview-handler DLL. Version 0.1.0; 10,297 Rust source and test
lines (including blank lines and comments). See [testing](docs/TESTING.md).

Offline verification, with `PATH=/home/box/.cargo/bin:$PATH`,
`CARGO_NET_OFFLINE=true`, and `CARGO_TARGET_DIR` set to a scratch directory:

- `cargo fmt --check`: passed.
- `cargo test --offline`: passed, 116 tests, 0 failed, 0 ignored.
- `cargo build --offline`: passed in the prior verification.
- `cargo clippy --offline --all-targets`: passed with warnings in the prior verification.
- `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu`:
  passed with warnings.
- `cargo build --offline --release --target x86_64-pc-windows-gnu`: passed.
- `cargo test --offline --no-run --target x86_64-pc-windows-gnu`: passed;
  test binaries were not executed (prior verification).
- `DIST_DIR=/tmp/terra-review/07a-csv-viewer/dist scripts/build-windows.sh`:
  passed in the prior verification and produced three binaries and SHA256SUMS.

Real Windows Explorer, COM stream threading, registration conflicts and cleanup,
DPI, resizing, worker failure and packaging still need testing. Unregistration
removes matching values and cleans up candidate keys only when empty. Registration
errors can leave partial values. The preview COM stream's cross-thread use requires
marshalling review. The text preview currently uses a fixed layout width in the
Explorer host. See `docs/TESTING.md` before distribution.
