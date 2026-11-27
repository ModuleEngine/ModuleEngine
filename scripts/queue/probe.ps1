$started = [DateTimeOffset]::UtcNow.ToString('o')
Write-Output "Probe started $started in Windows session $((Get-Process -Id $PID).SessionId). No renderer was launched."
Start-Sleep -Seconds 25
Write-Output "Probe finished $([DateTimeOffset]::UtcNow.ToString('o'))."
