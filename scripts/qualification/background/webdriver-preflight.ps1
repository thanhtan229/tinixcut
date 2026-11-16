$ErrorActionPreference = 'Stop'

# Read-only: never start the application or take control of the interactive desktop.
$driver = Get-Command tauri-driver -ErrorAction SilentlyContinue
if (-not $driver) {
    $candidate = Join-Path $env:USERPROFILE '.cargo\bin\tauri-driver.exe'
    if (Test-Path -LiteralPath $candidate) { $driverPath = $candidate }
} else { $driverPath = $driver.Source }
$edgeDriver = Get-Command msedgedriver -ErrorAction SilentlyContinue
$cachedEdge = Join-Path $env:LOCALAPPDATA 'TinixCutTestDrivers\157.0.4341.57\msedgedriver.exe'
$edgePath = if ($edgeDriver) { $edgeDriver.Source } elseif (Test-Path -LiteralPath $cachedEdge) { $cachedEdge } else { $null }
$webviewRoot = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
$webviewVersions = if (Test-Path -LiteralPath $webviewRoot) {
    @(Get-ChildItem -LiteralPath $webviewRoot -Directory | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'msedgewebview2.exe') } | ForEach-Object { $_.Name })
} else { @() }
[pscustomobject]@{
    tauriDriver = if ($driverPath) { $driverPath } else { $null }
    edgeDriver = $edgePath
    edgeDriverVersion = if ($edgePath) { (& $edgePath --version | Out-String).Trim() } else { $null }
    webviewVersions = $webviewVersions
    isolatedDesktopVerified = $false
    note = 'Preflight only. A separate desktop driver status probe is available; an isolated Tauri application session and profile have not been verified.'
} | ConvertTo-Json -Depth 3
