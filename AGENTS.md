# Agent brief — CSV viewer and preview

Made by Maria. Licence: Apache-2.0 (LICENSE, NOTICE). Credit name is "Maria" only.

1. Follow docs/ARCHITECTURE.md. Implement **v1 scope only**; roadmap/future items stay out (write them in ROADMAP.md if useful).
2. MT5 / MetaTrader: **strictly read-only**. No order placement, modification, closing or any trading call. Not affiliated with MetaQuotes; MT5 is a MetaQuotes trademark.
3. Tests are required for every module: unit tests for core logic, golden/fixture tests for parsers. Work is done only when build and tests pass; report the exact commands and results in STATUS.md.
4. Rust: no Windows target is installed on this box (no rustup, no mingw). Keep Win32/COM code behind `#[cfg(windows)]` with traits so the core crate builds and tests on Linux (`cargo build`, `cargo test`). Do not install toolchains. List everything that needs real Windows in TESTING.md.
5. No network publishing: no git push, no git remote, no package publishing, no uploads, no telemetry. Network use at runtime only where the architecture requires it, opt-in and documented.
6. Stay inside this repository directory. Do not read or modify anything outside it (other than toolchain caches). Never read or print credentials.
7. Keep LICENSE and NOTICE unchanged. README must keep "Made by Maria" as its first line. UI apps show "Made by Maria" in an About screen. Store copy: "free and open source (Apache-2.0)". Add SPDX-License-Identifier: Apache-2.0 headers to source files. Third-party deps: MIT/BSD/ISC/Apache-2.0/Zlib only; record them in THIRD_PARTY.md; no GPL code.
8. Commit locally in small, meaningful commits (author Maria). At the end write STATUS.md: what is built, build/test commands and results, LOC, tests needing real Windows, open issues.
