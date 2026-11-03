#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Portable standalone developer/release payload; existing COM packaging is untouched.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" == --help ]]; then
  printf '%s\n' 'Usage: bash scripts/build-desktop.sh [--debug]' 'Default: release Windows GNU payload in dist/standalone; --debug builds a labelled development payload.'
  exit 0
fi
profile=release
flags=(--release)
if [[ "${1:-}" == --debug ]]; then profile=debug; flags=(); elif [[ $# -gt 0 ]]; then printf '%s\n' 'Unknown option' >&2; exit 2; fi
cargo build --locked --offline --target x86_64-pc-windows-gnu "${flags[@]}" -p csv-desktop -p csv-worker
python3 tools/build_docs.py --check
output=dist/standalone
mkdir -p "$output/fonts" "$output/help"
cp "${CARGO_TARGET_DIR:-target}/x86_64-pc-windows-gnu/$profile/csv-desktop.exe" "$output/"
cp "${CARGO_TARGET_DIR:-target}/x86_64-pc-windows-gnu/$profile/csv-worker.exe" "$output/"
cp LICENSE NOTICE THIRD_PARTY.md THIRD_PARTY_LICENSES.txt "$output/"
cp assets/desktop-fonts/* "$output/fonts/"
cp docs/USER-GUIDE.md "$output/README.md"
cp docs/USER-GUIDE.html "$output/help/index.html"
printf '%s\n' "$profile / unsigned Windows GNU build; native acceptance pending" > "$output/BUILD.txt"
(cd "$output"; sha256sum csv-desktop.exe csv-worker.exe LICENSE NOTICE THIRD_PARTY.md THIRD_PARTY_LICENSES.txt README.md BUILD.txt fonts/* help/* > SHA256SUMS; sha256sum -c SHA256SUMS)
rm -f "dist/csv-desktop-$profile-windows.zip"
(cd "$output"; zip -q -r "../csv-desktop-$profile-windows.zip" csv-desktop.exe csv-worker.exe fonts help LICENSE NOTICE THIRD_PARTY.md THIRD_PARTY_LICENSES.txt README.md BUILD.txt SHA256SUMS)
