#Requires -Version 7.5
[CmdletBinding()]
param(
    [string]$QueueRoot = 'E:\ModuleEngine-queue',
    [string]$Python,
    [ValidateRange(1024, 65535)][int]$Port = 16924,
    [switch]$RegisterAutostart
)

. "$PSScriptRoot/common.ps1"
$QueueRoot = Get-QueueRoot $QueueRoot
$pueue = Get-QueueTool pueue
$daemon = Get-QueueTool pueued
foreach ($tool in @($pueue, $daemon)) {
    $version = & $tool --version
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '^pueued? 4\.0\.4$') {
        throw "This queue was qualified for Pueue 4.0.4. Found '$version' at $tool."
    }
}

New-Item -ItemType Directory -Force -Path $QueueRoot | Out-Null
$runtime = Join-Path $QueueRoot 'runtime.json'
if ($Python) {
    $Python = (Get-Item -LiteralPath $Python).FullName
    & $Python -I -c 'import sys; raise SystemExit(sys.version_info < (3, 9, 17))'
    if ($LASTEXITCODE -ne 0) { throw 'Select a working Python 3.9.17 or newer.' }
}
if (-not (Test-Path -LiteralPath $runtime)) {
    if (-not $Python) { throw 'Initial setup requires the absolute path to a real Python executable.' }
    Write-QueueJson $runtime @{ python = $Python }
} elseif ($Python -and (Read-QueueJson $runtime).python -ne $Python) {
    throw "Python is already configured in $runtime. Leave it unchanged until the queue is drained."
}
$pwsh = (Get-Process -Id $PID).Path
$config = Join-Path $QueueRoot 'pueue.yml'
$daemonArgs = "--config ""$config"""
if (-not (Test-Path -LiteralPath $config)) {
    Write-QueueJson $config @{
        shared = @{
            pueue_directory = Join-Path $QueueRoot 'state'
            runtime_directory = Join-Path $QueueRoot 'state'
            host = '127.0.0.1'
            port = "$Port"
        }
        daemon = @{
            shell_command = @($pwsh, '-NoLogo', '-NoProfile', '-NonInteractive', '-EncodedCommand',
                '{{ pueue_command_string }}')
        }
    }
}
$settings = Read-QueueJson $config
if ($settings.shared.pueue_directory -ne (Join-Path $QueueRoot 'state') -or
    $settings.shared.host -ne '127.0.0.1' -or
    $settings.daemon.shell_command[-1] -cne '{{ pueue_command_string }}' -or
    $settings.daemon.shell_command[-2] -cne '-EncodedCommand') {
    throw "Unexpected dedicated Pueue configuration in $config."
}

if ($RegisterAutostart) {
    $name = 'ModuleEngine Local Check Queue'
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $user = $identity.Name
    $daemonLiteral = "'" + $daemon.Replace("'", "''") + "'"
    $configLiteral = "'" + $config.Replace("'", "''") + "'"
    $command = "& $daemonLiteral --config $configLiteral; exit " + '$LASTEXITCODE'
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $taskArgs = "-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded"
    $task = Get-ScheduledTask -TaskName $name -TaskPath '\' -ErrorAction SilentlyContinue
    if ($task) {
        if ($task.Actions.Execute -ne $pwsh -or $task.Actions.Arguments -cne $taskArgs -or
            ([Security.Principal.NTAccount]::new($task.Principal.UserId)).Translate([Security.Principal.SecurityIdentifier]) -ne $identity.User -or
            $task.Principal.LogonType -ne 'Interactive' -or
            @($task.Triggers).Count -ne 1 -or
            $task.Triggers[0].CimClass.CimClassName -ne 'MSFT_TaskLogonTrigger' -or
            ([Security.Principal.NTAccount]::new($task.Triggers[0].UserId)).Translate([Security.Principal.SecurityIdentifier]) -ne $identity.User -or
            -not $task.Triggers[0].Enabled) {
            throw "Existing task '$name' has different settings. Inspect it before proceeding."
        }
    } else {
        $action = New-ScheduledTaskAction -Execute $pwsh -Argument $taskArgs -WorkingDirectory $QueueRoot
        $trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
        $principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
        $taskSettings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -Hidden
        Register-ScheduledTask -TaskName $name -TaskPath '\' -Action $action -Trigger $trigger -Principal $principal -Settings $taskSettings | Out-Null
    }
    if ((Get-ScheduledTask -TaskName $name -TaskPath '\').State -ne 'Running') {
        Start-ScheduledTask -TaskName $name -TaskPath '\'
    }
}

$state = $null
for ($attempt = 0; $attempt -lt 15 -and -not $state; $attempt++) {
    $rawState = & $pueue --config $config status --json 2>$null
    if ($LASTEXITCODE -eq 0) { $state = $rawState | ConvertFrom-Json -AsHashtable -DateKind String }
    else { Start-Sleep -Seconds 1 }
}
if (-not $state) { throw "Start the dedicated daemon with '& $daemon $daemonArgs' from the interactive user session." }

foreach ($group in @('gpu', 'light')) {
    if (-not $state.groups.Contains($group)) { Invoke-Pueue group add $group | Out-Host }
    $limit = if ($group -eq 'gpu') { 1 } else { 2 }
    if ($state.groups[$group].parallel_tasks -ne $limit) {
        Invoke-Pueue parallel $limit --group $group | Out-Host
    }
}
Assert-QueueGroups (Get-QueueState)
Invoke-Pueue group
