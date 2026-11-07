# SPDX-License-Identifier: Apache-2.0
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-build-flags.ps1')
$separator = [string][char]0x1f
$source = 'C:\Users\Example\CSV Viewer'
$expected = "--remap-path-prefix=$source=/source"
$actual = Get-EncodedRustBuildFlags -SourceRoot $source
if ($actual -ne $expected) { throw 'Spaced checkout path was split or changed' }
$actual = Get-EncodedRustBuildFlags -SourceRoot $source -PlainFlags '  -C debuginfo=1  '
$parts = @($actual -split $separator)
if ($parts.Count -ne 3 -or $parts[0] -ne '-C' -or $parts[1] -ne 'debuginfo=1' -or $parts[2] -ne $expected) {
    throw 'Existing whitespace-delimited flags were not preserved'
}
$encoded = "-C${separator}link-arg=path with spaces"
$actual = Get-EncodedRustBuildFlags -SourceRoot $source -EncodedFlags $encoded -PlainFlags 'must-not-be-used'
$parts = @($actual -split $separator)
if ($parts.Count -ne 3 -or $parts[1] -ne 'link-arg=path with spaces' -or $parts[2] -ne $expected) {
    throw 'Encoded flags lost precedence or embedded spaces'
}
Write-Output 'PASS: checkout spaces, existing flags and encoded precedence'
