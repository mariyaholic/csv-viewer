# SPDX-License-Identifier: Apache-2.0
function Get-EncodedRustBuildFlags {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [string]$EncodedFlags = '',
        [string]$PlainFlags = ''
    )
    $separator = [string][char]0x1f
    $flags = @()
    if ($EncodedFlags) {
        $flags = @($EncodedFlags -split $separator)
    } elseif ($PlainFlags) {
        # Cargo splits ordinary RUSTFLAGS on whitespace. Preserve that behavior.
        $flags = @($PlainFlags -split '\s+' | Where-Object { $_ })
    }
    $flags += "--remap-path-prefix=$SourceRoot=/source"
    return ($flags -join $separator)
}
