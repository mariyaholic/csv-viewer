# SPDX-License-Identifier: Apache-2.0
# Creates a reviewable MSVC standalone candidate only. Never publishes.
param(
    [ValidateSet('debug', 'release')][string]$Profile = 'release',
    [string]$OutputDirectory = 'dist'
)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$commit = git rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Cannot obtain source commit' }
git diff-index --quiet HEAD --
if ($LASTEXITCODE -ne 0) { throw 'Commit tracked source changes before building a candidate' }
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
if ($LASTEXITCODE -ne 0) { throw 'Cannot obtain commit timestamp' }
. (Join-Path $PSScriptRoot 'rust-build-flags.ps1')
$env:CARGO_ENCODED_RUSTFLAGS = Get-EncodedRustBuildFlags -SourceRoot (Get-Location).Path -EncodedFlags $env:CARGO_ENCODED_RUSTFLAGS -PlainFlags $env:RUSTFLAGS
cargo fetch --locked
if ($LASTEXITCODE -ne 0) { throw 'Cannot fetch locked dependencies' }
python scripts/generate-third-party-licenses.py --check
if ($LASTEXITCODE -ne 0) { throw 'Third-party licence bundle is stale' }
python tools/build_docs.py --check
if ($LASTEXITCODE -ne 0) { throw 'Offline guide is stale' }
$buildArgs = @('build', '--locked', '--offline', '--target', 'x86_64-pc-windows-msvc', '-p', 'csv-desktop', '-p', 'csv-worker')
if ($Profile -eq 'release') { $buildArgs += '--release' }
cargo @buildArgs
if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$binaryDirectory = Join-Path $target "x86_64-pc-windows-msvc/$Profile"
python scripts/package-desktop.py --binary-dir $binaryDirectory --output-dir $OutputDirectory --commit $commit --profile $Profile
if ($LASTEXITCODE -ne 0) { throw 'Standalone packaging failed' }
