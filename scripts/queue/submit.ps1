#Requires -Version 7.5
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Worktree,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$Commit,
    [Parameter(Mandatory)][string]$Check,
    [ValidateSet('gpu', 'light')][string]$Group,
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]{0,31}$')][string]$Author = 'local',
    [string]$QueueRoot = 'E:\ModuleEngine-queue'
)

. "$PSScriptRoot/common.ps1"
$QueueRoot = Get-QueueRoot $QueueRoot
$spec = Get-QueueCheck $Check
if (-not $Group) { $Group = $spec.group }
if ($Group -eq 'light' -and $spec.group -ne 'light') { throw "'$Check' requires the gpu group." }
Assert-QueueGroups (Get-QueueState)

$gitOverrides = @(& git rev-parse --local-env-vars)
if ($LASTEXITCODE -ne 0) { throw 'Cannot determine Git environment overrides.' }
foreach ($name in $gitOverrides) {
    # Git transports per-command configuration through these variables, including Codex's Git defaults.
    if ($name -in @('GIT_CONFIG_COUNT', 'GIT_CONFIG_PARAMETERS')) { continue }
    if ([Environment]::GetEnvironmentVariable($name)) { throw "Unset $name before submitting." }
}
$Commit = $Commit.ToLowerInvariant()
$Worktree = (Resolve-Path -LiteralPath $Worktree).ProviderPath
$gitTop = Invoke-QueueGit -C $Worktree rev-parse --show-toplevel
if ($Worktree -ne [IO.Path]::GetFullPath($gitTop)) { throw 'Worktree must be the root of a Git checkout.' }
$common = [IO.Path]::GetFullPath((Invoke-QueueGit -C $Worktree rev-parse --path-format=absolute --git-common-dir))
$observed = Invoke-QueueGit -C $Worktree rev-parse --verify HEAD
$token = [guid]::NewGuid().ToString('N')
$requestDir = Join-Path $QueueRoot "requests/$token"
$snapshot = Join-Path $QueueRoot "snapshots/$token"
$label = "{0}:{1}@{2}" -f $Author, $Check, $Commit.Substring(0, 7)
$runner = "'" + (Join-Path $PSScriptRoot 'run.ps1').Replace("'", "''") + "'"
$rootArg = "'" + $QueueRoot.Replace("'", "''") + "'"
$payload = "& $runner -QueueRoot $rootArg -Token $token; exit " + '$LASTEXITCODE'
$command = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($payload))

New-Item -ItemType Directory -Force -Path $requestDir, (Split-Path -Parent $snapshot) | Out-Null
if ($observed -eq $Commit) {
    # Store the exact commit in its own linked-worktree HEAD before queuing. Checkout happens in the slot.
    Invoke-QueueGit -C $Worktree worktree add --detach --no-checkout $snapshot $Commit | Out-Null
}
$request = @{
    schema_version = 1
    token = $token
    author = $Author
    check = $Check
    group = $Group
    commit = $Commit
    submitted_head = $observed
    submitted_at = [DateTimeOffset]::UtcNow.ToString('o')
    worktree = $Worktree
    git_common_dir = $common
    command = $command
    check_registry_sha256 = (Get-FileHash -LiteralPath "$PSScriptRoot/checks.json").Hash.ToLowerInvariant()
}
try {
    # Allocate the authoritative ID, then publish the request before it can run.
    $addArgs = @('--stashed', '--print-task-id', '--group', $Group, '--label', $label,
        '--working-directory', $QueueRoot, '--', $command)
    $id = Add-PueueTask $addArgs
    if ($id -isnot [string] -or $id -notmatch '^\d+$') { throw "Invalid Pueue task ID: $id" }
    $request.id = [uint64]$id
    $resultDir = Join-Path $QueueRoot "results/$id"
    New-Item -ItemType Directory -Path $resultDir | Out-Null
    Write-QueueJson (Join-Path $resultDir 'request.json') $request
    Write-QueueJson (Join-Path $requestDir 'task.json') @{ id = $request.id }
    Invoke-Pueue enqueue $id | Out-Null
} catch {
    throw "Submission could not be confirmed (task ID '$id', request '$token'). Inspect Pueue before retrying. $_"
}
$id
