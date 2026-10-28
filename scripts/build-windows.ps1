# SPDX-License-Identifier: Apache-2.0
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
if ($LASTEXITCODE -ne 0) { throw 'Cannot obtain commit timestamp' }
$env:RUSTFLAGS = "$env:RUSTFLAGS --remap-path-prefix=$((Get-Location).Path)=/source"
cargo build --locked --release --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
New-Item -ItemType Directory -Force dist | Out-Null
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$files = @('csv-viewer.exe', 'csv-worker.exe', 'csv_preview.dll')
foreach ($file in $files) { Copy-Item (Join-Path $target "x86_64-pc-windows-msvc/release/$file") dist }
$hashes = foreach ($file in $files) { $hash = (Get-FileHash (Join-Path dist $file) -Algorithm SHA256).Hash.ToLowerInvariant(); "$hash  $file" }
[IO.File]::WriteAllLines((Join-Path (Get-Location) 'dist/SHA256SUMS'), [string[]]$hashes, [Text.UTF8Encoding]::new($false))
