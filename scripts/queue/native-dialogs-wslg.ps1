#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Artifacts,
    [string]$LinuxToolchain = 'E:/ModuleEngine/.tools/linux-x86_64/env.sh',
    [string]$LinuxAlsaSysroot = 'E:/ModuleEngine-queue/tools/r6-wsl-alsa/sysroot',
    [string]$LinuxZenitySysroot = 'E:/ModuleEngine-queue/tools/r6-wsl-zenity/sysroot'
)

$ErrorActionPreference = 'Stop'
$Artifacts = (Resolve-Path -LiteralPath $Artifacts).ProviderPath
& wsl.exe --distribution Ubuntu --user root --exec bash -euc `
    'cd "$(wslpath -u "$6")"; exec timeout --kill-after=15s 5100s bash "$(wslpath -u "$1")" "$(wslpath -u "$2")" "$(wslpath -u "$3")" "$(wslpath -u "$4")" "$(wslpath -u "$5")"' `
    -- "$PSScriptRoot/native-dialogs-wslg.sh" $Artifacts $LinuxToolchain $LinuxAlsaSysroot $LinuxZenitySysroot (Get-Location).ProviderPath
exit $LASTEXITCODE
