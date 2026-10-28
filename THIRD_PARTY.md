# Third-party software

Made by Maria. Apache-2.0.

This inventory covers the Rust dependencies recorded in `Cargo.lock`. Licences
were read from crate metadata and must be re-checked before a release. No
GPL-only code is used. Where a crate offers a choice, the allowed option is
selected below (for example, MIT instead of Unlicense, Apache-2.0 instead of
BSL-1.0).

## Direct dependencies

| Crate | Version | Declared licence | Notes |
| --- | --- | --- | --- |
| `csv` | 1.4.0 | Unlicense/MIT | Used under MIT. Proven CSV parser. |
| `encoding_rs` | 0.8.35 | (Apache-2.0 OR MIT) AND BSD-3-Clause | Used under Apache-2.0/MIT; BSD-3-Clause notice retained. |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Protocol, settings, report JSON. |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | Protocol and golden reports. |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | Private bounded spool and settings writes. |
| `crossterm` | 0.29.0 | MIT | Companion TUI. |
| `unicode-width` | 0.2.3 | MIT OR Apache-2.0 | Table/text display width. |

## Transitive dependencies

| Crate | Version | Declared licence |
| --- | --- | --- |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |
| `crossterm_winapi` | 0.9.1 | MIT |
| `csv-core` (crates.io) | 0.1.13 | Unlicense/MIT |
| `document-features` | 0.2.12 | MIT OR Apache-2.0 |
| `errno` | 0.3.14 | MIT OR Apache-2.0 |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 |
| `libc` | 0.2.191 | MIT OR Apache-2.0 |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `litrs` | 1.0.1 | MIT OR Apache-2.0 |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 |
| `log` | 0.4.34 | MIT OR Apache-2.0 |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `mio` | 1.2.4 | MIT |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| `redox_syscall` | 0.5.18 | MIT |
| `rustix` | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `ryu` | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |
| `signal-hook` | 0.3.18 | Apache-2.0/MIT |
| `signal-hook-mio` | 0.2.5 | MIT OR Apache-2.0 |
| `signal-hook-registry` | 1.4.8 | MIT OR Apache-2.0 |
| `smallvec` | 1.16.4 | MIT OR Apache-2.0 |
| `syn` | 3.0.9 | MIT OR Apache-2.0 |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `winapi` | 0.3.9 | MIT/Apache-2.0 |
| `winapi-i686-pc-windows-gnu` | 0.4.0 | MIT/Apache-2.0 |
| `winapi-x86_64-pc-windows-gnu` | 0.4.0 | MIT/Apache-2.0 |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `zmij` | 1.0.23 | MIT |

## Review notes

- `csv`, `csv-core` (crates.io) and `memchr` are dual Unlicense/MIT; this
  project relies on the MIT option.
- `ryu` is dual Apache-2.0/BSL-1.0; this project relies on Apache-2.0.
- `r-efi` is tri-licensed including LGPL-2.1-or-later. It is pulled in only as
  a target-specific dependency of `getrandom` for UEFI; it is not built for
  Linux or Windows. This project relies on the MIT/Apache-2.0 options and does
  not link LGPL code.
- `unicode-ident` also carries the permissive Unicode-3.0 data licence. It is a
  transitive build-time dependency of `serde_derive`/`syn`; the MIT/Apache-2.0
  options are used for the code. The Unicode data licence is not GPL and should
  be included in the final release review.
- `winapi*`, `windows-sys`, `windows-link`, `crossterm_winapi`, `wasi`,
  `redox_syscall` and `r-efi` are target-specific and not built on Linux.
- `LICENSE` and `NOTICE` are unchanged; this file is the third-party notice
  inventory required by the project brief.
