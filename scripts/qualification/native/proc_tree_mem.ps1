# Sum memory for tinixcut.exe and all descendants (WebView2 processes, ffmpeg).
$all = Get-CimInstance Win32_Process
$root = $all | Where-Object { $_.Name -eq 'tinixcut.exe' } | Select-Object -First 1
$ids = @($root.ProcessId); $frontier = @($root.ProcessId)
while ($frontier.Count) { $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId } | ForEach-Object ProcessId); $ids += $next; $frontier = $next }
$procs = Get-Process -Id $ids -ErrorAction SilentlyContinue
[pscustomobject]@{ processes=$procs.Count; privateMiB=[math]::Round(($procs | Measure-Object PrivateMemorySize64 -Sum).Sum/1MB,1); workingSetMiB=[math]::Round(($procs | Measure-Object WorkingSet64 -Sum).Sum/1MB,1); names=(($procs | Group-Object ProcessName | ForEach-Object { "$($_.Name)x$($_.Count)" }) -join ',') } | ConvertTo-Json -Compress
