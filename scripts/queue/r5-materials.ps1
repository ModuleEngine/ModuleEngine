#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Artifacts,
    [Parameter(Mandatory)][string]$FixtureRoot
)

$ErrorActionPreference = 'Stop'
$snapshot = (Get-Location).Path
$artifactsRoot = (Resolve-Path -LiteralPath $Artifacts).Path
$fixtures = (Resolve-Path -LiteralPath $FixtureRoot).Path
$dxc = (Resolve-Path -LiteralPath $env:MODULE_ENGINE_DXC).Path
$full = Join-Path $artifactsRoot 'full'
$evidence = Join-Path $artifactsRoot 'material-check'
$archive = Join-Path $env:CARGO_TARGET_DIR 'r5-material-source.tar'
if (Test-Path -LiteralPath $archive) { throw "Source archive already exists: $archive" }

New-Item -ItemType Directory -Path $full | Out-Null
& git -C $snapshot archive --format=tar --output=$archive HEAD
if ($LASTEXITCODE -ne 0) { throw "git archive failed: $LASTEXITCODE" }
& tar.exe -xf $archive -C $full
if ($LASTEXITCODE -ne 0) { throw "Extracting the Full source delivery failed: $LASTEXITCODE" }

$dxcDirectory = Split-Path -Parent $dxc
foreach ($file in @($dxc, (Join-Path $dxcDirectory 'dxcompiler.dll'), (Join-Path $dxcDirectory 'dxil.dll'))) {
    Copy-Item -LiteralPath $file -Destination (Join-Path $full 'tools/dxc')
}
$built = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc/release/module-engine-editor.exe'
$editor = Join-Path $full 'ModuleEngine.exe'
Copy-Item -LiteralPath $built -Destination $editor
$editorHash = (Get-FileHash -LiteralPath $editor).Hash.ToLowerInvariant()
if ($editorHash -cne (Get-FileHash -LiteralPath $built).Hash.ToLowerInvariant()) {
    throw 'The retained Editor differs from the built executable.'
}
$binaryProof = @{
    editor_sha256 = $editorHash
    full_source_tar_sha256 = (Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant()
    dxc_sha256 = (Get-FileHash -LiteralPath $dxc).Hash.ToLowerInvariant()
}
$binaryProofFile = Join-Path $artifactsRoot 'material-binaries.json'
$binaryProof | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath $binaryProofFile
& $editor --seal-source-project $full
if ($LASTEXITCODE -ne 0) { throw "Sealing the Full source delivery failed: $LASTEXITCODE" }
$project = Join-Path $full 'r5-material-source.json'
if (Test-Path -LiteralPath $project) { throw "Verification starter already exists: $project" }
& $editor --new-game $project
if ($LASTEXITCODE -ne 0) { throw "Creating the current SDK project failed: $LASTEXITCODE" }
& $editor --material-check $project $fixtures $evidence
if ($LASTEXITCODE -ne 0) { throw "Material verification exited with $LASTEXITCODE" }
