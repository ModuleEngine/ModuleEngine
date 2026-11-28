#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Artifacts,
    [Parameter(Mandatory)][string]$FixtureRoot
)

$ErrorActionPreference = 'Stop'
$cache = Join-Path (Get-Location).Path '.tools/bench-assets/sponza'
$fixture = (Resolve-Path -LiteralPath $FixtureRoot).Path
$manifest = Get-Content -Raw -LiteralPath 'examples/bench/sponza/manifest.json' | ConvertFrom-Json
New-Item -ItemType Directory -Path $cache | Out-Null
foreach ($item in $manifest.files) {
    if ($item.path -notmatch '^[A-Za-z0-9_.-]+$' -or $item.path -in '.','..') {
        throw "Unexpected Sponza file name: $($item.path)"
    }
    $from = Join-Path $fixture $item.path
    if ((Get-FileHash -LiteralPath $from).Hash.ToLowerInvariant() -cne $item.sha256) {
        throw "Sponza input hash differs: $($item.path)"
    }
    Copy-Item -LiteralPath $from -Destination (Join-Path $cache $item.path)
}

$artifactsRoot = (Resolve-Path -LiteralPath $Artifacts).Path
$built = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc/release/module-engine-editor.exe'
$editor = Join-Path $artifactsRoot 'ModuleEngine.exe'
Copy-Item -LiteralPath $built -Destination $editor
$sha = (Get-FileHash -LiteralPath $editor).Hash.ToLowerInvariant()
if ($sha -cne (Get-FileHash -LiteralPath $built).Hash.ToLowerInvariant()) {
    throw 'Retained Editor hash differs from the built executable'
}
[pscustomobject]@{ editor_sha256 = $sha; sponza_revision = $manifest.commit } |
    ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $artifactsRoot 'lighting-binary.json')
& $editor --bench-scene R52-lighting (Join-Path $artifactsRoot 'r52')
if ($LASTEXITCODE -ne 0) { throw "The S4 lighting check exited with $LASTEXITCODE" }
