#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][uint64]$Id,
    [switch]$FullLog,
    [string]$QueueRoot = 'E:\ModuleEngine-queue'
)

. "$PSScriptRoot/common.ps1"
$QueueRoot = Get-QueueRoot $QueueRoot
$directory = Join-Path $QueueRoot "results/$Id"
$requestPath = Join-Path $directory 'request.json'
$reportPath = Join-Path $directory 'report.json'
$logPath = Join-Path $directory 'log.txt'
if (-not (Test-Path -LiteralPath $requestPath -PathType Leaf)) { throw "No ModuleEngine request for ID $Id." }
$request = Read-QueueJson $requestPath
if ($request.id -ne $Id) { throw "Request ID does not match $Id." }

if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
    $state = Get-QueueState
    $task = $state.tasks["$Id"]
    if (-not $task -or $task.command -cne $request.command -or $task.group -ne $request.group) {
        throw "Task $Id has no report and its original Pueue entry is unavailable. See $directory."
    }
    try {
        Invoke-Pueue wait --quiet "$Id" | Out-Null
    } catch {
        if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
            throw "Could not wait for $Id. The request and any partial log are in $directory. $_"
        }
    }
    if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
        $task = (Get-QueueState).tasks["$Id"]
        if ($task -and $task.command -ceq $request.command) {
            $status = $task.status | ConvertTo-Json -Depth 10 -Compress
            throw "Task $Id ended without a finalized report (Pueue: $status). See $directory and Pueue log $Id. Resubmit for a fresh ID."
        }
        throw "Task $Id has no finalized report. See $directory."
    }
}

$report = Read-QueueJson $reportPath
if ($report.id -ne $Id -or $report.token -cne $request.token -or $report.commit -cne $request.commit) {
    throw "Report identity does not match request $Id."
}
Get-Content -Raw -LiteralPath $reportPath
if ($FullLog) { Get-Content -LiteralPath $logPath -Encoding utf8 }
else { Get-Content -LiteralPath $logPath -Encoding utf8 -Tail 50 }
if ($report.status -ne 'passed') { exit 1 }
