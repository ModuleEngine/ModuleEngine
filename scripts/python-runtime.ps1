function Find-ModuleEnginePython([string]$Python) {
    $candidates = @()
    if ($Python) {
        if (-not [IO.Path]::IsPathRooted($Python)) {
            throw '-Python / MODULE_ENGINE_VERIFY_PYTHON must name an absolute Python executable.'
        }
        $candidates += [pscustomobject]@{Path = $Python; Prefix = @()}
    } else {
        foreach ($name in @('py', 'python3', 'python')) {
            foreach ($command in @(Get-Command $name -CommandType Application -All -ErrorAction SilentlyContinue)) {
                $prefix = @()
                if ($name -eq 'py') { $prefix = @('-3') }
                $candidates += [pscustomobject]@{Path = $command.Source; Prefix = $prefix}
            }
        }
    }
    $diagnostic = $null
    foreach ($candidate in $candidates) {
        if ($candidate.Path -match '(?i)[\\/]WindowsApps[\\/]' -or
            -not (Test-Path -LiteralPath $candidate.Path -PathType Leaf)) { continue }
        try {
            $prefix = @($candidate.Prefix)
            $result = @(& $candidate.Path @prefix -B (Join-Path $PSScriptRoot 'python_runtime.py') 2>&1)
            if ($LASTEXITCODE -eq 0 -and $result.Count -eq 1 -and
                [IO.Path]::IsPathRooted($result[0]) -and
                $result[0] -notmatch '(?i)[\\/]WindowsApps[\\/]' -and
                (Test-Path -LiteralPath $result[0] -PathType Leaf)) {
                return $result[0]
            }
            if (-not $diagnostic -and $LASTEXITCODE -eq 2) {
                $diagnostic = $result -join [Environment]::NewLine
            }
        } catch {
            # A broken launcher cannot supply a usable interpreter.
        }
    }
    if ($diagnostic) { throw $diagnostic }
    throw 'Python 3.9.17 or newer with tarfile.data_filter is required. Pass -Python <absolute-python.exe> or set MODULE_ENGINE_VERIFY_PYTHON. WindowsApps aliases are ignored.'
}
