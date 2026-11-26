param(
    [string]$Baseline,
    [string]$OutputRoot
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent $PSScriptRoot
$previousInput = $env:MODULE_ENGINE_R15_INPUT
$previousRun = $env:MODULE_ENGINE_R15_RUN

function Assert-Hash([string]$Path, [string]$Expected) {
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -cne $Expected) {
        throw "Unexpected SHA-256 in $Path (expected $Expected, got $actual)"
    }
}

function Assert-OrderedEvents([string]$Reference, [string]$Candidate) {
    $before = [IO.File]::OpenText($Reference)
    try {
        $after = [IO.File]::OpenText($Candidate)
        try {
            while (!$before.EndOfStream -or !$after.EndOfStream) {
                $expected = $before.ReadLine()
                $actual = $after.ReadLine()
                if ($expected -cne $actual) {
                    $line = if ($null -ne $expected) { $expected } else { $actual }
                    $entry = ConvertFrom-Json -InputObject $line
                    throw "First event-log difference at $($entry.segment), scene $($entry.scene), tick $($entry.tick). Compare $Reference and $Candidate."
                }
            }
        } finally { $after.Dispose() }
    } finally { $before.Dispose() }
}

function Invoke-Check([string]$Log, [string[]]$Arguments) {
    "cargo $($Arguments -join ' ')" | Tee-Object -FilePath $Log
    & cargo @Arguments 2>&1 | Tee-Object -FilePath $Log -Append
    if ($LASTEXITCODE -ne 0) { throw "Cargo exited with $LASTEXITCODE. See $Log." }
    if (!(Select-String -LiteralPath $Log -Pattern '1 passed; 0 failed' -Quiet)) {
        throw "Expected exactly one successful test. See $Log."
    }
}

Push-Location -LiteralPath $root
try {
    $expected = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'r15-replay-baseline.json') -Raw | ConvertFrom-Json
    Assert-Hash 'game-project.json' $expected.game_project_sha256
    $defaultBaseline = Join-Path $root 'outputs/r15/baseline'
    if (!$Baseline -and (Test-Path -LiteralPath $defaultBaseline -PathType Container)) { $Baseline = $defaultBaseline }
    $env:MODULE_ENGINE_R15_INPUT = $null
    if ($Baseline) {
        $Baseline = (Resolve-Path -LiteralPath $Baseline).ProviderPath
        $env:MODULE_ENGINE_R15_INPUT = Join-Path $Baseline 'input.json'
        Assert-Hash $env:MODULE_ENGINE_R15_INPUT $expected.input_sha256
        Assert-Hash (Join-Path $Baseline 'events.ndjson') $expected.events_sha256
    }
    if (!$OutputRoot) { $OutputRoot = Join-Path $root "outputs/r15/replay-$(Get-Date -Format 'yyyyMMdd-HHmmss-fff')" }
    $OutputRoot = (New-Item -ItemType Directory -Path $OutputRoot).FullName
    $revision = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Cannot determine the source revision.' }
    $rust = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'Cannot determine the Rust toolchain.' }
    if ($rust -notcontains "host: $($expected.target)") {
        throw "The replay baseline was recorded for $($expected.target)."
    }
    @("HEAD $revision", "Baseline $($expected.revision)", $rust) | Set-Content (Join-Path $OutputRoot 'source.txt')
    $replay = @('test', '--offline', '--locked', '--release', '--target', $expected.target, '-p', 'module-engine-editor', '--features', 'verification',
        'schedule_replay::r15_schedule_event_replay', '--', '--exact', '--ignored', '--nocapture', '--test-threads=1')
    foreach ($run in @('first', 'repeat')) {
        $env:MODULE_ENGINE_R15_RUN = Join-Path $OutputRoot $run
        Invoke-Check (Join-Path $OutputRoot "$run.log") $replay
        $inputPath = Join-Path $env:MODULE_ENGINE_R15_RUN 'input.json'
        $eventPath = Join-Path $env:MODULE_ENGINE_R15_RUN 'events.ndjson'
        Assert-Hash $inputPath $expected.input_sha256
        if ($Baseline) { Assert-OrderedEvents (Join-Path $Baseline 'events.ndjson') $eventPath }
        if ($run -eq 'repeat') { Assert-OrderedEvents (Join-Path $OutputRoot 'first/events.ndjson') $eventPath }
        Assert-Hash $eventPath $expected.events_sha256
        $env:MODULE_ENGINE_R15_INPUT = $inputPath
    }
    Invoke-Check (Join-Path $OutputRoot 'rollback.log') @('test', '--offline', '--locked', '--release', '--target', $expected.target,
        '-p', 'module-engine-host', '--lib', 'neutral_tests::r14_tick_rejection_discards_visible_writes_and_retry_starts_at_last_commit',
        '--', '--exact', '--test-threads=1')
    "Both event logs match $($expected.revision), and the GameHost rollback check passed." |
        Tee-Object -FilePath (Join-Path $OutputRoot 'result.txt')
    "Evidence: $OutputRoot"
} finally {
    $env:MODULE_ENGINE_R15_INPUT = $previousInput
    $env:MODULE_ENGINE_R15_RUN = $previousRun
    Pop-Location
}
