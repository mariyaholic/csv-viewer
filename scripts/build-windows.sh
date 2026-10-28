#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_NET_OFFLINE=true
export SOURCE_DATE_EPOCH="$(git log -1 --format=%ct)"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$(pwd)=/source"
cargo build --offline --locked --release --target x86_64-pc-windows-gnu
out="${DIST_DIR:-dist}"
mkdir -p "$out"
for file in csv-viewer.exe csv-worker.exe csv_preview.dll; do
  cp "${CARGO_TARGET_DIR:-target}/x86_64-pc-windows-gnu/release/$file" "$out/$file"
done
(cd "$out" && sha256sum csv-viewer.exe csv-worker.exe csv_preview.dll > SHA256SUMS)
