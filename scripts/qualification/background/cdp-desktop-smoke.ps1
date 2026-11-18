param([switch]$Recovery, [switch]$SavedRecovery, [switch]$Workflow, [switch]$Pointer, [switch]$Controls, [switch]$Audio, [switch]$GainPreview, [switch]$Captions, [switch]$Media, [switch]$HistoryBranch, [ValidateRange(1,600)][int]$TimeoutSeconds=300)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'cdp-diagnostics.ps1')
$chosen=@('Recovery','SavedRecovery','Workflow','Pointer','Controls','Audio','GainPreview','Captions','Media','HistoryBranch' | Where-Object {Get-Variable -Name $_ -ValueOnly})
if($chosen.Count -gt 1){throw 'Select only one CDP mode'}
$mode=if($chosen.Count){$chosen[0]}else{'Smoke'}
New-Item -ItemType Directory -Path (Join-Path $PWD.Path 'artifacts/background') -Force | Out-Null
$receipt=if($env:TINIXCUT_CDP_SESSION_REPORT){$env:TINIXCUT_CDP_SESSION_REPORT}else{Join-Path $PWD.Path ('artifacts/background/cdp-session-'+$mode+'.json')}
$session=[ordered]@{startedAt=[DateTime]::UtcNow.ToString('o');mode=$mode;status='running';functionalStatus='not-run';stages=@();killAttempts=@();diagnostics=@()}
function Save-Session { $session | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $receipt -Encoding UTF8 }
function Add-CdpCheckpoint([string]$CheckpointLabel) {
 try{$session.diagnostics+=@(Get-CdpSnapshot $session $CheckpointLabel)}
 catch{$session.diagnostics+=@([ordered]@{label=$CheckpointLabel;at=[DateTime]::UtcNow.ToString('o');error=$_.Exception.Message})}
 Save-Session
}
function Stop-TestTree([int]$OwnedId) {
 $killInfo=New-Object Diagnostics.ProcessStartInfo
 $killInfo.FileName=Join-Path $env:SystemRoot 'System32/taskkill.exe'
 $killInfo.Arguments='/PID '+$OwnedId+' /T /F'
 $killInfo.UseShellExecute=$false;$killInfo.CreateNoWindow=$true
 $killInfo.RedirectStandardOutput=$true;$killInfo.RedirectStandardError=$true
 $killer=New-Object Diagnostics.Process;$killer.StartInfo=$killInfo
 try {
  [void]$killer.Start()
  $out=$killer.StandardOutput.ReadToEndAsync();$err=$killer.StandardError.ReadToEndAsync()
  if(-not $killer.WaitForExit(5000)){$killer.Kill();[void]$killer.WaitForExit(1000);$killCode=124}else{$killCode=$killer.ExitCode}
  $killLog=[string]::Concat($out.Result,$err.Result)
 } finally {$killer.Dispose()}
 Write-Output ('Cleanup process '+$OwnedId+': exit '+$killCode)
 $session.killAttempts+=@([ordered]@{processId=$OwnedId;exitCode=$killCode;detail=$killLog})
 # A child may exit between inventory and taskkill. Final process/profile
 # verification decides cleanup success, and all unsuccessful attempts remain recorded.
}
function Get-LiveProfileProcesses([string]$ProfilePath) {
 foreach($candidate in @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {$_.CommandLine -like "*$ProfilePath*"})){
  if([CdpReadOnlyProbe]::State($candidate.ProcessId) -ne 'exited'){Write-Output $candidate}
 }
}
function Stop-IsolatedWebView([string]$ProfilePath) {
 foreach($owned in @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {$_.CommandLine -like "*$ProfilePath*"})) {
  if([CdpReadOnlyProbe]::State($owned.ProcessId) -eq 'running'){Stop-TestTree $owned.ProcessId}
 }
}

Write-Output ('CDP preflight: '+$mode)
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

Write-Output 'CDP desktop interop ready'
$name='TinixCutCDP-'+[guid]::NewGuid().ToString('N')
$binary=if($env:TINIXCUT_CDP_BINARY){[IO.Path]::GetFullPath($env:TINIXCUT_CDP_BINARY)}else{Join-Path $PWD.Path 'src-tauri\target-qualification\debug\tinixcut.exe'}
if($env:TINIXCUT_SCOPE_PROBE_RUN -eq '1' -and (-not $Recovery -or -not $env:TINIXCUT_CDP_BINARY -or $env:TINIXCUT_CDP_WINDOW_TITLE -ne 'TinixCut qualification production identity')) {throw 'Scope qualification requires Recovery mode, explicit binary and qualification window title'}
if (Get-NetTCPConnection -State Listen -LocalPort 9444 -ErrorAction SilentlyContinue) {throw 'CDP port occupied'}
if (Get-CimInstance Win32_Process -Filter "Name='tinixcut.exe'" | Where-Object {$_.ExecutablePath -eq $binary}) {throw 'Qualification already running'}
Write-Output 'CDP port/app ownership checked'
$desktop=[DesktopProbe]::CreateDesktop($name,[IntPtr]::Zero,[IntPtr]::Zero,0,0x01ff,[IntPtr]::Zero)
if ($desktop -eq [IntPtr]::Zero) {throw 'CreateDesktop failed'}
$input=[DesktopProbe]::OpenInputDesktop(0,$false,1)
$before=[DesktopProbe]::Name($input); [void][DesktopProbe]::CloseDesktop($input)
$profile=Join-Path ([IO.Path]::GetTempPath()) $name
$fixtureDir=Join-Path ([IO.Path]::GetTempPath()) ('tinixcut-cdp-fixtures\'+$name)
New-Item -ItemType Directory -Path $fixtureDir -Force | Out-Null
$oldCdpProject=$env:TINIXCUT_CDP_PROJECT
$env:TINIXCUT_CDP_PROJECT=Join-Path $fixtureDir 'caption.tcproj'
$oldProfile=$env:WEBVIEW2_USER_DATA_FOLDER
$env:WEBVIEW2_USER_DATA_FOLDER=$profile
$oldQualificationRoot=$env:TINIXCUT_QUALIFICATION_DATA_ROOT
$oldScopeProbe=$env:TINIXCUT_SCOPE_PROBE
$scopeRoot=$null
$pi=New-Object DesktopProbe+PROCESS_INFORMATION
try {
if($env:TINIXCUT_SCOPE_PROBE_RUN -eq '1') {
 $scopeRoot=Join-Path ([IO.Path]::GetTempPath()) ('tinixcut-isolated-'+[guid]::NewGuid().ToString('N'))
 New-Item -ItemType Directory -Path $scopeRoot -ErrorAction Stop | Out-Null
 [IO.File]::WriteAllText((Join-Path $scopeRoot 'scope-fixture.txt'),'qualification fixture')
 $env:TINIXCUT_QUALIFICATION_DATA_ROOT=$scopeRoot
 $session.qualificationDataRoot=$scopeRoot
}
$hashStream=[IO.File]::OpenRead($binary);$hasher=[Security.Cryptography.SHA256]::Create()
try {$session.binarySha256=[BitConverter]::ToString($hasher.ComputeHash($hashStream)).Replace('-','')}finally{$hashStream.Dispose();$hasher.Dispose()}
$session.inputDesktop=$before
$session.desktop=$name
Save-Session
 foreach($stage in $(if($Recovery -or $SavedRecovery){@('prepare','resume')}else{@('smoke')})) {
 if($env:TINIXCUT_SCOPE_PROBE_RUN -eq "1") { if(-not $Recovery){throw "Scope probe requires Recovery mode"}; $env:TINIXCUT_SCOPE_PROBE=if($stage -eq "prepare"){"grant"}else{"verify"} }
 $si=New-Object DesktopProbe+STARTUPINFO; $si.cb=[Runtime.InteropServices.Marshal]::SizeOf($si);$si.desktop="winsta0\$name"
 $cmd=New-Object Text.StringBuilder ('"'+$binary+'"')
 if (-not [DesktopProbe]::CreateProcess($binary,$cmd,[IntPtr]::Zero,[IntPtr]::Zero,$false,0x08000000,[IntPtr]::Zero,$PWD.Path,[ref]$si,[ref]$pi)) {throw 'CreateProcess failed'}
 $ready=$false
 for($i=0;$i -lt 60;$i++) {
  $input=[DesktopProbe]::OpenInputDesktop(0,$false,1)
  try {if ([DesktopProbe]::Name($input) -ne $before) {throw 'Input desktop changed'}} finally {[void][DesktopProbe]::CloseDesktop($input)}
  try {$pages=Invoke-RestMethod http://127.0.0.1:9444/json/list -TimeoutSec 1; if($pages){$ready=$true;break}} catch {}
  Start-Sleep -Milliseconds 250
 }
 if(-not $ready) {throw 'CDP did not become ready on isolated desktop'}
 $listeners=@(Get-NetTCPConnection -State Listen -LocalPort 9444)
 if($listeners | Where-Object { $_.LocalAddress -notin @('127.0.0.1','::1') }){throw 'CDP is not loopback-only'}
 foreach($listener in $listeners){
   $owner=Get-CimInstance Win32_Process -Filter ("ProcessId="+$listener.OwningProcess)
   if($owner.CommandLine -notlike "*$profile*"){throw 'CDP listener does not belong to isolated profile'}
 }
 $session.processId=$pi.processId
 $appOwner=Get-CimInstance Win32_Process -Filter ('ProcessId='+$pi.processId)
 if(-not $appOwner -or [IO.Path]::GetFullPath($appOwner.ExecutablePath) -ne $binary){throw 'Launched process binary mismatch'}
 if(-not [DesktopProbe]::HasWindow($desktop,$pi.processId)){throw 'App window not on isolated desktop'}
 & node scripts/qualification/background/cdp-identity.mjs
 if($LASTEXITCODE -ne 0){throw 'Qualification window identity mismatch'}
 $script=if($HistoryBranch){'cdp-history-branch.mjs'}elseif($Captions){'cdp-captions.mjs'}elseif($Media){'cdp-media.mjs'}elseif($GainPreview){'cdp-gain-preview.mjs'}elseif($Audio){'cdp-audio.mjs'}elseif($Controls){'cdp-controls.mjs'}elseif($Pointer){'cdp-pointer.mjs'}elseif($Workflow){'cdp-workflow.mjs'}elseif($SavedRecovery){'cdp-saved-recovery.mjs'}elseif($Recovery){'cdp-recovery.mjs'}else{'cdp-readback.mjs'}
 $nodeArgs=@(('scripts/qualification/background/'+$script),$stage)
 $nodeInfo=New-Object Diagnostics.ProcessStartInfo
 $nodeInfo.FileName=(Get-Command node.exe).Source
 $nodeInfo.Arguments=($nodeArgs -join ' ')
 $nodeInfo.WorkingDirectory=$PWD.Path
 $nodeInfo.UseShellExecute=$false;$nodeInfo.CreateNoWindow=$true
 $nodeInfo.RedirectStandardOutput=$true;$nodeInfo.RedirectStandardError=$true
 $child=New-Object Diagnostics.Process;$child.StartInfo=$nodeInfo
 [void]$child.Start()
 $outTask=$child.StandardOutput.ReadToEndAsync();$errTask=$child.StandardError.ReadToEndAsync()
 $timedOut=-not $child.WaitForExit($TimeoutSeconds*1000)
 if($timedOut){ Stop-TestTree $child.Id; $child.WaitForExit(5000) | Out-Null }
 $child.Refresh()
 $stageResult=[ordered]@{stage=$stage;timedOut=$timedOut;exitCode=$child.ExitCode;stdout=[string]$outTask.Result;stderr=[string]$errTask.Result;loopbackOwned=$true;windowIsolated=$true}
 $session.stages+=@($stageResult);Save-Session
 if($stageResult.stdout){Write-Output $stageResult.stdout};if($stageResult.stderr){Write-Output $stageResult.stderr}
 if($timedOut){throw ('CDP stage timed out after '+$TimeoutSeconds+' seconds')}
 if($child.ExitCode -ne 0){throw 'CDP readback failed'}
 if($scopeRoot) {
  $scopeMarkerName=if($stage -eq 'prepare'){'scope-grant.ok'}else{'scope-verify.ok'}
  $scopeMarker=Join-Path $scopeRoot $scopeMarkerName
  $scopeFile=Join-Path $scopeRoot 'plugin/.persisted-scope-asset'
  if(-not (Test-Path -LiteralPath $scopeMarker -PathType Leaf) -or -not (Test-Path -LiteralPath $scopeFile -PathType Leaf)) {throw 'Scope probe marker or persisted scope is missing'}
  if([IO.File]::ReadAllText($scopeMarker) -ne (Join-Path $scopeRoot 'scope-fixture.txt')) {throw 'Scope probe fixture path mismatch'}
  $stageResult.scopeVerified=$true
 }
 $input=[DesktopProbe]::OpenInputDesktop(0,$false,1)
 try {if ([DesktopProbe]::Name($input) -ne $before) {throw 'Input desktop changed during CDP readback'}} finally {[void][DesktopProbe]::CloseDesktop($input)}
 $stageResult.inputDesktopUnchanged=$true;Save-Session
 Write-Output ('Verified input desktop remained '+$before+'; app window stayed on '+$name)
 if($stage -eq 'prepare') {
   Stop-TestTree $pi.processId
   Stop-IsolatedWebView $profile
   [void][DesktopProbe]::CloseHandle($pi.process); [void][DesktopProbe]::CloseHandle($pi.thread)
   $pi=New-Object DesktopProbe+PROCESS_INFORMATION
   for($wait=0;$wait -lt 30;$wait++) {
     $remaining=@(Get-LiveProfileProcesses $profile)
     if($remaining.Count -eq 0){break}
     Start-Sleep -Milliseconds 250
   }
   if($remaining.Count -gt 0){throw 'Profile WebView processes remain after stop'}
   Start-Sleep -Milliseconds 1500
 }
 }
 $session.status='passed';$session.functionalStatus='passed'
} catch {
 $session.status='failed';$session.functionalStatus='failed';$session.error=$_.Exception.Message
 throw
} finally {
 try {
 Add-CdpCheckpoint 'before-close'
 if($pi.processId){
  $closeInfo=New-Object Diagnostics.ProcessStartInfo
  $closeInfo.FileName=(Get-Command node.exe).Source
  $closeInfo.Arguments='scripts/qualification/background/cdp-close.mjs'
  $closeInfo.WorkingDirectory=$PWD.Path
  $closeInfo.UseShellExecute=$false;$closeInfo.CreateNoWindow=$true
  $closer=New-Object Diagnostics.Process;$closer.StartInfo=$closeInfo
  try{[void]$closer.Start();if(-not $closer.WaitForExit(12000)){$closer.Kill();$session.gracefulClose=$false}else{$session.gracefulClose=($closer.ExitCode -eq 0)}}finally{$closer.Dispose()}
  for($exitWait=0;$exitWait -lt 20;$exitWait++){[uint32]$nativeExit=259;[void][DesktopProbe]::GetExitCodeProcess($pi.process,[ref]$nativeExit);if($nativeExit -ne 259){break};Start-Sleep -Milliseconds 250}
 }
 $session.nativeExitCode=$nativeExit
 Add-CdpCheckpoint 'after-app-exit-wait'
 if($pi.processId -and $nativeExit -eq 259){$ownedRoot=Get-CimInstance Win32_Process -Filter ('ProcessId='+$pi.processId);if($ownedRoot.ExecutablePath -eq $binary){Stop-TestTree $pi.processId}}
 for($browserWait=0;$browserWait -lt 20;$browserWait++){
  $browserRemaining=@(Get-LiveProfileProcesses $profile)
  if($browserRemaining.Count -eq 0){break}
  Start-Sleep -Milliseconds 250
 }
 if($browserRemaining.Count -gt 0){Stop-IsolatedWebView $profile}
 Add-CdpCheckpoint 'after-browser-exit-wait'
 if($pi.process -ne [IntPtr]::Zero){[void][DesktopProbe]::CloseHandle($pi.process)}
 if($pi.thread -ne [IntPtr]::Zero){[void][DesktopProbe]::CloseHandle($pi.thread)}
 [void][DesktopProbe]::CloseDesktop($desktop)
 if($child){$child.Dispose()}
 if($owner){$owner.Dispose();$owner=$null}
 if($ownedRoot){$ownedRoot.Dispose();$ownedRoot=$null}
 [GC]::Collect();[GC]::WaitForPendingFinalizers();[GC]::Collect()
 if($null -eq $oldProfile){Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue}else{$env:WEBVIEW2_USER_DATA_FOLDER=$oldProfile}
 if($null -eq $oldQualificationRoot){Remove-Item Env:TINIXCUT_QUALIFICATION_DATA_ROOT -ErrorAction SilentlyContinue}else{$env:TINIXCUT_QUALIFICATION_DATA_ROOT=$oldQualificationRoot}
 if($null -eq $oldScopeProbe){Remove-Item Env:TINIXCUT_SCOPE_PROBE -ErrorAction SilentlyContinue}else{$env:TINIXCUT_SCOPE_PROBE=$oldScopeProbe}
 $tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
 if([IO.Path]::GetFullPath($profile) -ne (Join-Path $tempRoot $name) -or [IO.Path]::GetFullPath($fixtureDir) -ne (Join-Path $tempRoot ('tinixcut-cdp-fixtures\'+$name))){throw 'Cleanup path outside expected temporary directory'}
 Add-CdpCheckpoint 'before-profile-removal'
 for($attempt=0;$attempt -lt 3 -and (Test-Path -LiteralPath $profile);$attempt++) {
   try {Remove-Item -LiteralPath $profile -Recurse -Force} catch {if($attempt -eq 2){throw}; Start-Sleep -Milliseconds 250}
 }
 if($null -eq $oldCdpProject){Remove-Item Env:TINIXCUT_CDP_PROJECT -ErrorAction SilentlyContinue}else{$env:TINIXCUT_CDP_PROJECT=$oldCdpProject}
 if(Test-Path -LiteralPath $fixtureDir){Remove-Item -LiteralPath $fixtureDir -Recurse -Force}
 $remainingProfile=@(Get-LiveProfileProcesses $profile)
 $session.cleanup=[ordered]@{passed=((-not (Test-Path -LiteralPath $profile)) -and (-not (Test-Path -LiteralPath $fixtureDir)) -and $remainingProfile.Count -eq 0 -and ([CdpReadOnlyProbe]::State([uint32]$pi.processId) -eq 'exited'));profileRemoved=(-not (Test-Path -LiteralPath $profile));fixtureRemoved=(-not (Test-Path -LiteralPath $fixtureDir));profileProcessCount=$remainingProfile.Count}
 if(-not $session.cleanup.passed){throw 'Owned CDP resources remain'}
 } catch {$session.status='failed';$session.cleanup=[ordered]@{passed=$false;error=$_.Exception.Message};Add-CdpCheckpoint 'cleanup-failed';throw}
 finally {$session.finishedAt=[DateTime]::UtcNow.ToString('o');Save-Session}
}
