param([switch]$DriverSmoke, [switch]$AppSmoke, [switch]$Diagnostic, [switch]$Control)
$ErrorActionPreference = 'Stop'

# The probe never switches desktops or synthesizes input. App modes launch only the qualification binary.
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class DesktopProbe {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct STARTUPINFO {
    public int cb; public string reserved; public string desktop; public string title;
    public int x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reserved2;
    public IntPtr reservedPtr, stdin, stdout, stderr;
  }
  [StructLayout(LayoutKind.Sequential)] public struct PROCESS_INFORMATION {
    public IntPtr process, thread; public int processId, threadId;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr devmode, int flags, uint access, IntPtr security);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool CloseDesktop(IntPtr desktop);
  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr OpenInputDesktop(int flags, bool inherit, uint access);
  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr GetThreadDesktop(int threadId);
  public delegate bool EnumProc(IntPtr window, IntPtr data);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool EnumDesktopWindows(IntPtr desktop, EnumProc callback, IntPtr data);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool GetUserObjectInformation(IntPtr obj, int index, StringBuilder info, int length, out int needed);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool CreateProcess(string app, StringBuilder command, IntPtr pa, IntPtr ta, bool inherit, int flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GetExitCodeProcess(IntPtr process, out uint code);
  public static string Name(IntPtr desktop) { int needed; var b = new StringBuilder(256); if (!GetUserObjectInformation(desktop, 2, b, 512, out needed)) throw new System.ComponentModel.Win32Exception(); return b.ToString(); }
  public static bool HasWindow(IntPtr desktop, uint processId) { bool found = false; EnumProc callback = (w, p) => { uint id; GetWindowThreadProcessId(w, out id); if (id == processId) found = true; return true; }; if (!EnumDesktopWindows(desktop, callback, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception(); return found; }
}
'@
$name = 'TinixCutProbe-' + [guid]::NewGuid().ToString('N')
$input = [DesktopProbe]::OpenInputDesktop(0, $false, 0x0001)
if ($input -eq [IntPtr]::Zero) { throw "OpenInputDesktop failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
$before = [DesktopProbe]::Name($input)
[void][DesktopProbe]::CloseDesktop($input)
$desktop = [DesktopProbe]::CreateDesktop($name, [IntPtr]::Zero, [IntPtr]::Zero, 0, 0x01ff, [IntPtr]::Zero)
if ($desktop -eq [IntPtr]::Zero) { throw "CreateDesktop failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
$pi = New-Object DesktopProbe+PROCESS_INFORMATION
$probeFile = Join-Path ([IO.Path]::GetTempPath()) ("$name.txt")
$webviewDir = Join-Path ([IO.Path]::GetTempPath()) $name
$diagnosticDir = Join-Path $PWD.Path 'artifacts\phase59\isolated-webdriver'
$binary = $null
$binaryName = 'tinixcut.exe'
$ownsApp = $false
try {
    $si = New-Object DesktopProbe+STARTUPINFO
    $si.cb = [Runtime.InteropServices.Marshal]::SizeOf($si)
    $si.desktop = "winsta0\$name"
    $exe = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $script = @'
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System; using System.Text; using System.Runtime.InteropServices; public class OwnDesktop { [DllImport("kernel32.dll")] public static extern int GetCurrentThreadId(); [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(int id); [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool GetUserObjectInformation(IntPtr h,int i,StringBuilder b,int n,out int needed); }'
$f=New-Object Windows.Forms.Form
$f.Text='TinixCut isolated desktop probe'
$f.Show()
$b=New-Object Text.StringBuilder 256; $needed=0
if (-not [OwnDesktop]::GetUserObjectInformation([OwnDesktop]::GetThreadDesktop([OwnDesktop]::GetCurrentThreadId()),2,$b,512,[ref]$needed)) { exit 2 }
[IO.File]::WriteAllText($env:TINIXCUT_PROBE_FILE,$b.ToString())
Start-Sleep -Seconds 5
$f.Close()
'@
    $env:TINIXCUT_PROBE_FILE = $probeFile
    if ($Control) { $Diagnostic = $true; $binaryName = 'tinixcut-webdriver-control.exe'; $env:TINIXCUT_PROBE_CONTROL = '1' }
    if ($Diagnostic) { $AppSmoke = $true; New-Item -ItemType Directory -Path $diagnosticDir -Force | Out-Null; $env:TINIXCUT_PROBE_DIAGNOSTIC = $diagnosticDir; $env:TINIXCUT_PROBE_LOG_PREFIX = if ($Control) { 'control' } else { 'tinixcut' } }
    if ($DriverSmoke -or $AppSmoke) {
        $tauriDriver = Join-Path $env:USERPROFILE '.cargo\bin\tauri-driver.exe'
        $edgeDriver = Join-Path $env:LOCALAPPDATA 'TinixCutTestDrivers\157.0.4341.57\msedgedriver.exe'
        if (-not (Test-Path -LiteralPath $tauriDriver) -or -not (Test-Path -LiteralPath $edgeDriver)) { throw 'Required Tauri or Edge driver is missing' }
        $env:TINIXCUT_PROBE_TAURI_DRIVER = $tauriDriver
        $env:TINIXCUT_PROBE_EDGE_DRIVER = $edgeDriver
        if ($AppSmoke) {
            $binary = Join-Path $PWD.Path "src-tauri\target-qualification\debug\$binaryName"
            if (-not (Test-Path -LiteralPath $binary)) { throw 'Qualification binary is missing; build it first' }
            if (Get-CimInstance Win32_Process -Filter "Name='$binaryName'" | Where-Object { $_.ExecutablePath -eq $binary }) { throw 'A qualification app is already running; refusing to share it' }
            $ownsApp = $true
            $env:TINIXCUT_PROBE_BINARY = $binary
            $env:WEBVIEW2_USER_DATA_FOLDER = $webviewDir
        }
        $script += @'

if (Get-NetTCPConnection -State Listen -LocalPort 14444,14445 -ErrorAction SilentlyContinue) { exit 4 }
if ($env:TINIXCUT_PROBE_DIAGNOSTIC) {
  $env:TAURI_AUTOMATION='true'; $env:TAURI_WEBVIEW_AUTOMATION='true'
  $edgeLog=Join-Path $env:TINIXCUT_PROBE_DIAGNOSTIC ($env:TINIXCUT_PROBE_LOG_PREFIX + '-edge-verbose.log')
  $driver=Start-Process -FilePath $env:TINIXCUT_PROBE_EDGE_DRIVER -ArgumentList @('--port=14444','--verbose',('--log-path="' + $edgeLog + '"')) -PassThru -NoNewWindow
} else {
  $driver = Start-Process -FilePath $env:TINIXCUT_PROBE_TAURI_DRIVER -ArgumentList @('--port','14444','--native-port','14445','--native-driver',('"' + $env:TINIXCUT_PROBE_EDGE_DRIVER + '"')) -PassThru -NoNewWindow
}
try {
  $ok=$false
  for ($i=0; $i -lt 30; $i++) {
    if ($driver.HasExited) { exit 5 }
    try { $result=Invoke-RestMethod -Uri 'http://127.0.0.1:14444/status' -TimeoutSec 1; $listener=Get-NetTCPConnection -State Listen -LocalPort 14444 -ErrorAction SilentlyContinue; if ($null -ne $result.value -and $listener.OwningProcess -eq $driver.Id) { $ok=$true; break } } catch {}
    Start-Sleep -Milliseconds 200
  }
  if (-not $ok) { exit 3 }
  [IO.File]::WriteAllText(($env:TINIXCUT_PROBE_FILE + '.driver'),'ok')
  if ($env:TINIXCUT_PROBE_BINARY) {
    if ($env:TINIXCUT_PROBE_DIAGNOSTIC) {
      $body = @{capabilities=@{alwaysMatch=@{browserName='webview2'; 'ms:edgeChromium'=$true; 'ms:edgeOptions'=@{binary=$env:TINIXCUT_PROBE_BINARY;args=@()}}}} | ConvertTo-Json -Depth 8 -Compress
    } else { $body = @{capabilities=@{alwaysMatch=@{browserName='wry'; 'tauri:options'=@{application=$env:TINIXCUT_PROBE_BINARY}}}} | ConvertTo-Json -Depth 8 -Compress }
    $created = Invoke-RestMethod -Uri 'http://127.0.0.1:14444/session' -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 45
    $session = $created.value.sessionId
    if (-not $session) { throw 'No WebDriver session ID returned' }
    try {
      $scriptBody = @{script='return document.title'; args=@()} | ConvertTo-Json -Compress
      $title=Invoke-RestMethod -Uri "http://127.0.0.1:14444/session/$session/execute/sync" -Method Post -ContentType 'application/json' -Body $scriptBody -TimeoutSec 10
      if (-not $title.value) { throw 'WebDriver returned no document title' }
      if (-not $env:TINIXCUT_PROBE_CONTROL) {
        $healthBody = @{script='const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke("app_health_check").then(v=>done({health:v})).catch(e=>done({error:String(e)}));';args=@()} | ConvertTo-Json -Compress
        $health = Invoke-RestMethod -Uri "http://127.0.0.1:14444/session/$session/execute/async" -Method Post -ContentType 'application/json' -Body $healthBody -TimeoutSec 15
        if ($health.value.error -or -not $health.value.health) { throw "Native health check failed: $($health.value | ConvertTo-Json -Compress)" }
      }
      [IO.File]::WriteAllText(($env:TINIXCUT_PROBE_FILE + '.app'),[string]$title.value)
      for ($j=0; $j -lt 80 -and -not (Test-Path -LiteralPath ($env:TINIXCUT_PROBE_FILE + '.continue')); $j++) { Start-Sleep -Milliseconds 250 }
      if (-not (Test-Path -LiteralPath ($env:TINIXCUT_PROBE_FILE + '.continue'))) { throw 'Parent did not verify application desktop' }
    } finally { Invoke-RestMethod -Uri "http://127.0.0.1:14444/session/$session" -Method Delete -TimeoutSec 10 | Out-Null }
  }
} finally { if (Get-Process -Id $driver.Id -ErrorAction SilentlyContinue) { & taskkill.exe /PID $driver.Id /T /F 2>$null | Out-Null } }
'@
    }
    $script = '$ErrorActionPreference="Stop"; try {' + "`n" + $script + "`n" + '} catch { [IO.File]::WriteAllText(($env:TINIXCUT_PROBE_FILE + ".error"),($_ | Out-String)); exit 1 }'
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
    $command = New-Object Text.StringBuilder ('"' + $exe + '" -NoProfile -EncodedCommand ' + $encoded)
    if (-not [DesktopProbe]::CreateProcess($exe, $command, [IntPtr]::Zero, [IntPtr]::Zero, $false, 0x08000000, [IntPtr]::Zero, $PWD.Path, [ref]$si, [ref]$pi)) { throw "CreateProcess failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
    for ($attempt=0; $attempt -lt 40 -and -not (Test-Path -LiteralPath $probeFile); $attempt++) { Start-Sleep -Milliseconds 250 }
    if (-not (Test-Path -LiteralPath $probeFile)) { throw 'Child did not report its desktop' }
    $assigned = [IO.File]::ReadAllText($probeFile)
    $input = [DesktopProbe]::OpenInputDesktop(0, $false, 0x0001)
    if ($input -eq [IntPtr]::Zero) { throw 'Cannot inspect input desktop during probe' }
    try { $during = [DesktopProbe]::Name($input) } finally { [void][DesktopProbe]::CloseDesktop($input) }
    if ($assigned -ne $name -or $before -ne $during) { throw "Isolation check failed: child=$assigned input=$before/$during" }
    if ($AppSmoke) {
        for ($attempt=0; $attempt -lt 240 -and -not (Test-Path -LiteralPath "$probeFile.app"); $attempt++) {
            if ($Diagnostic -and ($attempt % 8 -eq 0)) {
                $appSnapshot = @(Get-CimInstance Win32_Process -Filter "Name='$binaryName'" | Where-Object { $_.ExecutablePath -eq $binary })
                $appWindows = @($appSnapshot | ForEach-Object { [DesktopProbe]::HasWindow($desktop, $_.ProcessId) })
                $webviews = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$name*" })
                [pscustomobject]@{time=[DateTime]::UtcNow.ToString('o'); appPids=@($appSnapshot | ForEach-Object ProcessId); appWindowsOnProbe=$appWindows; webviewPids=@($webviews | ForEach-Object ProcessId)} | ConvertTo-Json -Compress -Depth 4 | Add-Content -LiteralPath (Join-Path $diagnosticDir ($env:TINIXCUT_PROBE_LOG_PREFIX + '-process-snapshots.jsonl')) -Encoding utf8
            }
            if (-not (Get-Process -Id $pi.processId -ErrorAction SilentlyContinue)) {
                $detail = if (Test-Path -LiteralPath "$probeFile.error") { [IO.File]::ReadAllText("$probeFile.error") } else { 'no child error captured' }
                throw "Probe child exited before application session was ready: $detail"
            }
            Start-Sleep -Milliseconds 250
        }
        if (-not (Test-Path -LiteralPath "$probeFile.app")) { throw 'Application session timed out' }
        $app = @(Get-CimInstance Win32_Process -Filter "Name='$binaryName'" | Where-Object { $_.ExecutablePath -eq $binary })
        if ($app.Count -ne 1 -or -not [DesktopProbe]::HasWindow($desktop, $app[0].ProcessId)) { throw 'Qualification app window is not on the isolated desktop' }
        $input = [DesktopProbe]::OpenInputDesktop(0, $false, 0x0001)
        if ($input -eq [IntPtr]::Zero) { throw 'Cannot inspect input desktop during application session' }
        try { $during = [DesktopProbe]::Name($input) } finally { [void][DesktopProbe]::CloseDesktop($input) }
        if ($during -ne $before) { throw 'Input desktop changed during application session' }
        [IO.File]::WriteAllText("$probeFile.continue", 'ok')
    }
    $process = Get-Process -Id $pi.processId -ErrorAction Stop
    if (-not $process.WaitForExit(90000)) { & taskkill.exe /PID $pi.processId /T /F | Out-Null; throw 'Probe timed out' }
    $exitCode = [uint32]0
    if (-not [DesktopProbe]::GetExitCodeProcess($pi.process, [ref]$exitCode) -or $exitCode -ne 0) { throw "Probe process exited with $exitCode" }
    if (($DriverSmoke -or $AppSmoke) -and -not (Test-Path -LiteralPath "$probeFile.driver")) { throw 'Driver status probe did not complete' }
    if ($AppSmoke -and -not (Test-Path -LiteralPath "$probeFile.app")) { throw 'Application WebDriver smoke did not complete' }
    [pscustomobject]@{ isolatedDesktop=$name; inputDesktop=$during; childDesktop=$assigned; probe='passed'; webdriver=$(if ($AppSmoke) { 'app session passed' } elseif ($DriverSmoke) { 'status passed; no app session' } else { 'not tested' }); title=$(if ($AppSmoke) { [IO.File]::ReadAllText("$probeFile.app") } else { $null }) } | ConvertTo-Json
} finally {
    Remove-Item Env:TINIXCUT_PROBE_FILE -ErrorAction SilentlyContinue
    Remove-Item Env:TINIXCUT_PROBE_TAURI_DRIVER,Env:TINIXCUT_PROBE_EDGE_DRIVER -ErrorAction SilentlyContinue
    Remove-Item Env:TINIXCUT_PROBE_BINARY,Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
    Remove-Item Env:TINIXCUT_PROBE_DIAGNOSTIC -ErrorAction SilentlyContinue
    Remove-Item Env:TINIXCUT_PROBE_CONTROL,Env:TINIXCUT_PROBE_LOG_PREFIX -ErrorAction SilentlyContinue
    if ($pi.processId -ne 0 -and (Get-Process -Id $pi.processId -ErrorAction SilentlyContinue)) { & taskkill.exe /PID $pi.processId /T /F 2>$null | Out-Null }
    if ($ownsApp) { Get-CimInstance Win32_Process -Filter "Name='$binaryName'" | Where-Object { $_.ExecutablePath -eq $binary } | ForEach-Object { & taskkill.exe /PID $_.ProcessId /T /F 2>$null | Out-Null } }
    Remove-Item -LiteralPath $probeFile -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$probeFile.driver" -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$probeFile.app" -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$probeFile.error" -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$probeFile.continue" -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $webviewDir) { Remove-Item -LiteralPath $webviewDir -Recurse -Force -ErrorAction SilentlyContinue }
    if ($pi.process -ne [IntPtr]::Zero) { [void][DesktopProbe]::CloseHandle($pi.process) }
    if ($pi.thread -ne [IntPtr]::Zero) { [void][DesktopProbe]::CloseHandle($pi.thread) }
    [void][DesktopProbe]::CloseDesktop($desktop)
}
