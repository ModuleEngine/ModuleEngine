#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('wasapi', 'wslg')][string]$Endpoint,
    [Parameter(Mandatory)][string]$Artifacts,
    [string]$LinuxToolchain = 'E:/ModuleEngine/.tools/linux-x86_64/env.sh',
    [string]$LinuxAlsaSysroot = 'E:/ModuleEngine-queue/tools/r6-wsl-alsa/sysroot'
)

$ErrorActionPreference = 'Stop'
$Artifacts = (Resolve-Path -LiteralPath $Artifacts).ProviderPath
$test = 'audio::tests::native_kira_loopback_receives_bound_cues_within_budget'
if ($Endpoint -eq 'wslg') {
    # Arguments are positional data. Convert the Windows paths in the guest.
    & wsl.exe --distribution Ubuntu --user root --exec bash -euc `
        'cd "$(wslpath -u "$6")"; exec timeout --kill-after=15s 3300s bash "$(wslpath -u "$1")" "$(wslpath -u "$2")" "$(wslpath -u "$3")" "$(wslpath -u "$4")" "$5"' `
        -- "$PSScriptRoot/audio-loopback-wslg.sh" $Artifacts $LinuxToolchain $LinuxAlsaSysroot $test (Get-Location).ProviderPath
    exit $LASTEXITCODE
}

& rustc -vV
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$executables = @( & cargo test --offline --locked --target x86_64-pc-windows-msvc `
    -p module-engine-host-services --features audio --lib --no-run --message-format=json | ForEach-Object {
        $message = $_ | ConvertFrom-Json
        if ($message.reason -eq 'compiler-artifact' -and $message.target.name -eq 'module_engine_host_services' `
            -and $message.profile.test -and $message.executable) {
            $message.executable
        } elseif ($message.reason -eq 'compiler-message' -and $message.message.rendered) {
            [Console]::Error.Write($message.message.rendered)
        }
    })
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if ($executables.Count -ne 1) { throw 'Expected one host-services audio test executable.' }
$probe = Join-Path $Artifacts 'audio-loopback-probe.exe'
Copy-Item -LiteralPath $executables[0] -Destination $probe
$hash = (Get-FileHash -LiteralPath $probe).Hash.ToLowerInvariant()
Write-Output "$hash  audio-loopback-probe.exe"
Set-Content -LiteralPath (Join-Path $Artifacts 'probe.sha256') -Value "$hash  audio-loopback-probe.exe"
$env:MODULE_ENGINE_AUDIO_PROBE_RECORDING = Join-Path $Artifacts 'capture.f32le'
& $probe $test --exact --ignored --nocapture --test-threads=1
$status = $LASTEXITCODE
if (Test-Path -LiteralPath $env:MODULE_ENGINE_AUDIO_PROBE_RECORDING -PathType Leaf) {
    $hash = (Get-FileHash -LiteralPath $env:MODULE_ENGINE_AUDIO_PROBE_RECORDING).Hash.ToLowerInvariant()
    Set-Content -LiteralPath (Join-Path $Artifacts 'capture.sha256') -Value "$hash  capture.f32le"
}
exit $status
