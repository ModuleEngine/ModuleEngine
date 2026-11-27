$ErrorActionPreference = 'Stop'

function Get-QueueRoot([string]$Path) {
    if (-not $IsWindows) { throw 'The ModuleEngine local queue requires PowerShell 7 on Windows.' }
    $fullPath = [IO.Path]::GetFullPath($Path)
    if ($fullPath -notmatch '^[A-Za-z]:\\' -or $fullPath -eq [IO.Path]::GetPathRoot($fullPath)) {
        throw 'QueueRoot must be an absolute local directory below a drive root.'
    }
    [IO.Path]::TrimEndingDirectorySeparator($fullPath)
}

function Get-QueueTool([string]$Name) {
    $installed = Join-Path $QueueRoot "bin/$Name.exe"
    if (Test-Path -LiteralPath $installed -PathType Leaf) { return $installed }
    (Get-Command "$Name.exe" -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
}

function Invoke-Pueue {
    $pueue = Get-QueueTool pueue
    $config = Join-Path $QueueRoot 'pueue.yml'
    if (-not (Test-Path -LiteralPath $config -PathType Leaf)) { throw "Run setup first: $config" }
    $output = & $pueue --config $config @args
    if ($LASTEXITCODE -ne 0) { throw "Pueue $($args[0]) failed (exit $LASTEXITCODE)." }
    $output
}

function Add-PueueTask([string[]]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new((Get-QueueTool pueue))
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in (@('--config', (Join-Path $QueueRoot 'pueue.yml'), 'add') + $Arguments)) {
        $start.ArgumentList.Add($argument)
    }
    # Pueue saves the client's entire environment and uses it to start the task.
    # Pass OS paths and the installed toolchain, leaving chat state and credentials out of its files.
    $start.Environment.Clear()
    foreach ($name in @('SystemRoot', 'SystemDrive', 'WINDIR', 'ComSpec', 'ProgramData', 'ProgramFiles',
        'ProgramFiles(x86)', 'ProgramW6432', 'CommonProgramFiles', 'CommonProgramFiles(x86)',
        'CommonProgramW6432', 'USERPROFILE', 'HOMEDRIVE', 'HOMEPATH', 'LOCALAPPDATA', 'APPDATA',
        'TEMP', 'TMP', 'USERNAME', 'USERDOMAIN', 'PROCESSOR_ARCHITECTURE', 'NUMBER_OF_PROCESSORS',
        'PATHEXT')) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ($value) { $start.Environment[$name] = $value }
    }
    $start.Environment['Path'] = (@(
        (Join-Path $QueueRoot 'bin'),
        [Environment]::GetEnvironmentVariable('Path', 'Machine'),
        [Environment]::GetEnvironmentVariable('Path', 'User')
    ) -join ';')
    $start.Environment['Path'] = [Environment]::ExpandEnvironmentVariables($start.Environment['Path'])
    $client = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $client.StandardOutput.ReadToEndAsync()
        $stderr = $client.StandardError.ReadToEndAsync()
        $client.WaitForExit()
        if ($client.ExitCode -ne 0) { throw "Pueue add failed (exit $($client.ExitCode)): $($stderr.Result.Trim())" }
        $stdout.Result.Trim()
    } finally {
        $client.Dispose()
    }
}

function Invoke-QueueGit {
    $output = & git.exe -c core.fsmonitor=false -c core.hooksPath=/dev/null @args
    if ($LASTEXITCODE -ne 0) { throw "Git failed (exit $LASTEXITCODE): $($args -join ' ')" }
    $output
}

function Assert-QueueGroups($State) {
    if ($State.groups.gpu.parallel_tasks -ne 1 -or $State.groups.light.parallel_tasks -ne 2) {
        throw 'Queue group limits differ from gpu=1/light=2. Run setup and check the daemon.'
    }
}

function Read-QueueJson([string]$Path) {
    Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -AsHashtable -DateKind String
}

function Write-QueueJson([string]$Path, $Value) {
    $tempPath = "$Path.$([guid]::NewGuid().ToString('N')).tmp"
    try {
        [IO.File]::WriteAllText($tempPath, ($Value | ConvertTo-Json -Depth 30))
        [IO.File]::Move($tempPath, $Path)
    } finally {
        if ([IO.File]::Exists($tempPath)) { [IO.File]::Delete($tempPath) }
    }
}

function Get-QueueCheck([string]$Name) {
    $checks = Read-QueueJson (Join-Path $PSScriptRoot 'checks.json')
    if (-not $checks.Contains($Name)) { throw "Unknown check '$Name'. Available: $($checks.Keys -join ', ')." }
    $checks[$Name]
}

function Get-QueueState {
    Invoke-Pueue status --json | ConvertFrom-Json -AsHashtable -DateKind String
}
