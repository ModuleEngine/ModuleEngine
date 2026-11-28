#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{32}$')][string]$Token,
    [Parameter(Mandatory)][string]$QueueRoot
)

. "$PSScriptRoot/common.ps1"
$QueueRoot = Get-QueueRoot $QueueRoot
$task = Read-QueueJson (Join-Path $QueueRoot "requests/$Token/task.json")
$id = [uint64]$task.id
$directory = Join-Path $QueueRoot "results/$id"
$request = Read-QueueJson (Join-Path $directory 'request.json')
$spec = Get-QueueCheck $request.check
$entry = (Get-QueueState).tasks["$id"]
if ($request.token -cne $Token -or $request.id -ne $id -or
    -not $entry -or $entry.command -cne $request.command -or
    $entry.group -ne $request.group -or $env:PUEUE_GROUP -ne $request.group -or
    $entry.status -isnot [System.Collections.IDictionary] -or -not $entry.status.Contains('Running') -or
    ($request.group -eq 'light' -and $spec.group -ne 'light')) {
    throw "Task $id is not the original running request in its permitted group."
}

$snapshot = Join-Path $QueueRoot "snapshots/$Token"
$artifacts = Join-Path $directory 'artifacts'
$log = Join-Path $directory 'log.txt'
$reportPath = Join-Path $directory 'report.json'
[IO.File]::Open($log, 'CreateNew').Dispose()
New-Item -ItemType Directory -Path $artifacts | Out-Null
$report = [ordered]@{
    schema_version = 1
    id = $id
    token = $Token
    author = $request.author
    check = $request.check
    group = $request.group
    commit = $request.commit
    submitted_head = $request.submitted_head
    worktree = $request.worktree
    snapshot = $snapshot
    submitted_at = $request.submitted_at
    started_at = [DateTimeOffset]::UtcNow.ToString('o')
    finished_at = $null
    check_started_at = $null
    check_finished_at = $null
    command = $spec.steps
    executed_commands = @()
    check_registry_sha256 = $request.check_registry_sha256
    window_executable_sha256 = $null
    timeout_seconds = $spec.timeout_seconds
    exit_code = $null
    status = 'failed'
    detail = $null
    source_digest_before = $null
    source_digest_after = $null
    environment = [ordered]@{
        os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
        session_id = (Get-Process -Id $PID).SessionId
        input_desktop_at_start = $null
        session_name = $env:SESSIONNAME
        icd_overrides = @{ VK_DRIVER_FILES = $env:VK_DRIVER_FILES; VK_ICD_FILENAMES = $env:VK_ICD_FILENAMES }
        dxc = $null
        video_controllers = @()
        render_mode = ($spec.render ?? 'none')
        actual_adapters = @()
        selected_adapter_evidence = @()
        gpu_evidence = 'not-run'
    }
    artifacts = $artifacts
    log = $log
}
$mutex = $null
$ownsMutex = $false

try {
    if ($request.submitted_head -cne $request.commit) {
        $report.status = 'wrong-commit'
        throw "The author checkout was at $($request.submitted_head) at submit, expected $($request.commit)."
    }
    if ((Get-FileHash -LiteralPath "$PSScriptRoot/checks.json").Hash -ne $request.check_registry_sha256) {
        throw 'The installed check registry changed after submission.'
    }
    if ($request.group -eq 'gpu') {
        $mutex = [Threading.Mutex]::new($false, 'Global\ModuleEngineLocalCheckGpu')
        try { $ownsMutex = $mutex.WaitOne(0) }
        catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
        if (-not $ownsMutex) { throw 'Another ModuleEngine queue runner owns the shared heavy slot.' }
    }
    $gitEntry = Join-Path $snapshot '.git'
    if (-not (Test-Path -LiteralPath $gitEntry -PathType Leaf) -or
        (Invoke-QueueGit -C $snapshot rev-parse --verify HEAD) -cne $request.commit -or
        [IO.Path]::GetFullPath((Invoke-QueueGit -C $snapshot rev-parse --path-format=absolute --git-common-dir)) -ne $request.git_common_dir -or
        @(Get-ChildItem -Force -LiteralPath $snapshot | Where-Object Name -ne '.git').Count -ne 0) {
        $report.status = 'wrong-commit'
        throw 'The pinned queue worktree is missing, populated ahead of execution, or at the wrong commit.'
    }
    $index = Invoke-QueueGit -C $snapshot rev-parse --path-format=absolute --git-path index
    if (Test-Path -LiteralPath $index) {
        $report.status = 'wrong-commit'
        throw 'The pinned queue worktree unexpectedly has an index before materialization.'
    }

    Invoke-QueueGit -C $snapshot -c core.sparseCheckout=false reset --hard $request.commit | Add-Content -LiteralPath $log
    if ((Invoke-QueueGit -C $snapshot rev-parse --verify HEAD) -cne $request.commit -or
        (Invoke-QueueGit -C $snapshot status --porcelain=v1 --untracked-files=all --ignore-submodules=none)) {
        $report.status = 'wrong-commit'
        throw 'The materialized checkout differs from the requested clean commit.'
    }

    $runtime = Read-QueueJson (Join-Path $QueueRoot 'runtime.json')
    $helper = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../review_snapshot.py'))
    $sourceCode = 'import json, runpy, sys; print(json.dumps(runpy.run_path(sys.argv[1])["capture"](sys.argv[2])))'
    $env:PYTHONUTF8 = '1'
    $env:PYTHONUNBUFFERED = '1'
    $env:CARGO_NET_OFFLINE = 'true'
    $env:CARGO_TARGET_DIR = Join-Path $snapshot 'target'
    $dxc = Join-Path $QueueRoot 'bin/dxc.exe'
    if ($spec.render -and (Test-Path -LiteralPath $dxc -PathType Leaf)) {
        $env:MODULE_ENGINE_DXC = $dxc
        $report.environment.dxc = @{ path = $dxc; sha256 = (Get-FileHash -LiteralPath $dxc).Hash.ToLowerInvariant() }
    }
    $before = (& $runtime.python -I -B -X utf8 -c $sourceCode $helper $snapshot | ConvertFrom-Json -AsHashtable)
    if ($LASTEXITCODE -ne 0 -or $before.head -cne $request.commit -or ($before.files.kind -contains 'missing')) {
        $report.status = 'wrong-commit'
        throw 'Cannot establish the complete source snapshot at the requested commit.'
    }
    Write-QueueJson (Join-Path $directory 'source.json') $before
    $report.source_digest_before = $before.source_digest

    Add-Type -Path "$PSScriptRoot/JobProcess.cs"
    $report.environment.input_desktop_at_start = [QueueJob]::HasInputDesktop()
    if ($spec.interactive -and -not $report.environment.input_desktop_at_start) {
        throw 'The check requires an unlocked interactive Windows desktop in the daemon session.'
    }
    if ($spec.render) {
        $report.environment.gpu_evidence = 'unverified'
        try {
            $report.environment.video_controllers = @(Get-CimInstance -ClassName Win32_VideoController -OperationTimeoutSec 5 |
                Select-Object Name, DriverVersion, PNPDeviceID, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate)
        } catch {
            Add-Content -LiteralPath $log "Cannot inventory display drivers: $_"
        }
    }

    $report.check_started_at = [DateTimeOffset]::UtcNow.ToString('o')
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        foreach ($step in $spec.steps) {
            $exe = switch ($step[0]) {
                'python' { $runtime.python }
                'pwsh' { (Get-Process -Id $PID).Path }
                'cargo' { (Get-Command cargo.exe -CommandType Application | Select-Object -First 1).Source }
                'window-probe' {
                    if (-not [QueueJob]::HasInputDesktop()) { throw 'The desktop is unavailable after the Release build.' }
                    $compiled = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc/release/examples/gpu_presentation_check.exe'
                    $saved = Join-Path $artifacts 'gpu_presentation_check.exe'
                    Copy-Item -LiteralPath $compiled -Destination $saved
                    $report.window_executable_sha256 = (Get-FileHash -LiteralPath $saved).Hash.ToLowerInvariant()
                    $saved
                }
                default { throw "Unexpected tool '$($step[0])' in registry." }
            }
            $stepArgs = @($step | Select-Object -Skip 1 | ForEach-Object {
                $_.Replace('{artifacts}', $artifacts).Replace('{tools}', $PSScriptRoot)
            })
            $report.executed_commands += ,(@($exe) + $stepArgs)
            Add-Content -LiteralPath $log ("Running " + (($report.executed_commands[-1]) | ConvertTo-Json -Compress))
            $remaining = [TimeSpan]::FromSeconds($spec.timeout_seconds) - $timer.Elapsed
            if ($remaining -gt [TimeSpan]::Zero) {
                $report.exit_code = [QueueJob]::Run($exe, $stepArgs, $snapshot, $log, $remaining)
            } else { $report.exit_code = $null }
            if ($null -eq $report.exit_code) {
                $report.status = 'timeout'
                throw "Exceeded the $($spec.timeout_seconds)-second check deadline. The job's processes were terminated."
            }
            if ($report.exit_code -ne 0) { throw "Check exited with code $($report.exit_code)." }
            if ($step[0] -eq 'window-probe' -and
                (Get-FileHash -LiteralPath $exe).Hash -ne $report.window_executable_sha256) {
                throw 'The window-probe executable changed during execution.'
            }
        }
        foreach ($required in $spec.artifacts) {
            if (-not (Test-Path -LiteralPath (Join-Path $artifacts $required) -PathType Leaf)) {
                throw "Required artifact '$required' is missing."
            }
        }
        $report.status = 'passed'
    } finally {
        $report.check_finished_at = [DateTimeOffset]::UtcNow.ToString('o')
        $after = (& $runtime.python -I -B -X utf8 -c $sourceCode $helper $snapshot | ConvertFrom-Json -AsHashtable)
        $report.source_digest_after = $after.source_digest
        if ($LASTEXITCODE -ne 0 -or $after.source_digest -cne $before.source_digest -or
            (Invoke-QueueGit -C $snapshot status --porcelain=v1 --untracked-files=no --ignore-submodules=none)) {
            if ($report.status -ne 'timeout') { $report.status = 'wrong-commit' }
            throw 'Source SHA, tracked state or source digest changed during execution or could not be rechecked.'
        }
    }
} catch {
    $report.detail = $_.Exception.Message
    Add-Content -LiteralPath $log ("ERROR: " + $_)
} finally {
    try {
        if ($spec.snapshot_artifacts -and $report.check_started_at) {
            foreach ($item in $spec.snapshot_artifacts) {
                $from = Join-Path $snapshot $item
                if (Test-Path -LiteralPath $from) {
                    $written = Get-Item -LiteralPath $from
                    if (($written.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
                        ($written.Parent.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                        throw "Expected a normal artifact directory under the queue snapshot: $from"
                    }
                    Move-Item -LiteralPath $from -Destination $artifacts
                }
            }
        }
        if ($request.check -eq 'window-graph' -and (Test-Path -LiteralPath (Join-Path $artifacts 'window.json'))) {
            $window = Read-QueueJson (Join-Path $artifacts 'window.json')
            $report.environment.actual_adapters = @($window.adapter | Where-Object { $_ })
            $report.environment.present_mode = $window.surface_mode
        }
        if ($request.check -eq 'bench-once') {
            $report.environment.actual_adapters = @(Get-BenchAdapters (Join-Path $artifacts 'bench'))
        }
        $materialReport = Join-Path $artifacts 'material-check/material-report.json'
        if ($request.check -eq 'r5-materials' -and (Test-Path -LiteralPath $materialReport -PathType Leaf)) {
            $materials = Read-QueueJson $materialReport
            $report.environment.actual_adapters = @($materials.captures.graphs.adapter | Sort-Object -Unique)
        }
        if ($report.environment.actual_adapters.Count) {
            $report.environment.selected_adapter_evidence = @($report.environment.actual_adapters | ForEach-Object {
                Get-AdapterEvidence $_ $report.environment.video_controllers
            })
            $classes = @($report.environment.selected_adapter_evidence.kind | Sort-Object -Unique)
            $report.environment.gpu_evidence = $classes -join ','
        }
    } catch {
        $report.status = if ($report.status -eq 'passed') { 'failed' } else { $report.status }
        $report.detail = (@($report.detail, "Artifact collection: $_") | Where-Object { $_ }) -join ' | '
        Add-Content -LiteralPath $log "Artifact collection: $_"
    } finally {
        $report.finished_at = [DateTimeOffset]::UtcNow.ToString('o')
        Write-QueueJson $reportPath $report
        if ($ownsMutex) { $mutex.ReleaseMutex() }
        if ($mutex) { $mutex.Dispose() }
    }
}

Write-Output "Task $id $($report.status). Report: $reportPath. Log: $log."
switch ($report.status) {
    'passed' { exit 0 }
    'wrong-commit' { exit 3 }
    'timeout' { exit 124 }
    default { exit 1 }
}
