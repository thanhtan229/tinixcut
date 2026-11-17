param([Parameter(Mandatory=$true)][string]$CandidateReport, [switch]$ResumeOwned, [ValidateSet('vanilla','accessibility','cdp')][string]$Mode='vanilla', [switch]$StandardUser)
$ErrorActionPreference='Stop'
if($ResumeOwned -and $Mode -ne 'vanilla'){throw 'ResumeOwned only supports the original vanilla receipt'}
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class InstalledDesktop {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]public struct SI {public int cb;public string reserved,desktop,title;public int x,y,xSize,ySize,xChars,yChars,fill,flags;public short show,reserved2;public IntPtr reservedPtr,stdin,stdout,stderr;}
 [StructLayout(LayoutKind.Sequential)]public struct PI {public IntPtr process,thread;public int id,threadId;}
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr CreateDesktop(string n,IntPtr d,IntPtr m,int f,uint a,IntPtr s);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr OpenDesktop(string n,int f,bool inherit,uint access);
 [DllImport("kernel32.dll",SetLastError=true)]public static extern IntPtr OpenProcess(uint access,bool inherit,int id);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr w);
 [DllImport("user32.dll")]public static extern bool CloseDesktop(IntPtr h);
 [DllImport("user32.dll")]public static extern IntPtr OpenInputDesktop(int f,bool i,uint a);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern bool GetUserObjectInformation(IntPtr h,int i,StringBuilder b,int l,out int needed);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)]public static extern bool CreateProcess(string a,StringBuilder c,IntPtr pa,IntPtr ta,bool inherit,int flags,IntPtr env,string cwd,ref SI si,out PI pi);
 [DllImport("kernel32.dll")]public static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll")]public static extern IntPtr GetCurrentProcess();
 [DllImport("advapi32.dll",SetLastError=true)]public static extern bool OpenProcessToken(IntPtr p,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)]public static extern bool GetTokenInformation(IntPtr token,int type,IntPtr data,int length,out int needed);
 [DllImport("advapi32.dll",SetLastError=true)]public static extern bool CreateRestrictedToken(IntPtr token,uint flags,int disable,IntPtr sids,int delete,IntPtr privileges,int restrict,IntPtr restrictSids,out IntPtr reduced);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]public static extern bool ConvertStringSidToSid(string text,out IntPtr sid);
 [DllImport("advapi32.dll",SetLastError=true)]public static extern bool SetTokenInformation(IntPtr token,int type,IntPtr data,int length);
 [DllImport("advapi32.dll")]public static extern int GetLengthSid(IntPtr sid);
 [DllImport("kernel32.dll")]public static extern IntPtr LocalFree(IntPtr p);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]public static extern bool CreateProcessAsUser(IntPtr token,string app,StringBuilder command,IntPtr pa,IntPtr ta,bool inherit,uint creation,IntPtr env,string cwd,ref SI si,out PI pi);
 public static string LaunchStandard(string app,StringBuilder cmd,string cwd,ref SI si,out PI pi){
  IntPtr token=IntPtr.Zero,linked=IntPtr.Zero,data=Marshal.AllocHGlobal(32),sid=IntPtr.Zero,env=IntPtr.Zero,acl=IntPtr.Zero;int needed;string kind="linked non-elevated current-user token";
  try{
   if(!OpenProcessToken(GetCurrentProcess(),0x8b,out token))throw new Exception("Cannot query current token");
   if(GetTokenInformation(token,19,data,IntPtr.Size,out needed)){linked=Marshal.ReadIntPtr(data);}
   else{
    if(!CreateRestrictedToken(token,4,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out linked))throw new Exception("CreateRestrictedToken: "+Marshal.GetLastWin32Error());
    if(!ConvertStringSidToSid("S-1-16-8192",out sid))throw new Exception("Medium integrity SID unavailable");
    Marshal.WriteIntPtr(data,sid);Marshal.WriteInt32(data,IntPtr.Size,0x20);
    if(!SetTokenInformation(linked,25,data,IntPtr.Size*2+GetLengthSid(sid)))throw new Exception("Set medium integrity: "+Marshal.GetLastWin32Error());
    // The filtered token cannot rely on Administrators ACEs for objects it
    // creates. Grant its own user and SYSTEM access in this token's default
    // DACL, allowing WebView2 to create its further restricted sandbox tokens.
    var user=System.Security.Principal.WindowsIdentity.GetCurrent().User;
    var descriptor=new System.Security.AccessControl.RawSecurityDescriptor("D:(A;;GA;;;SY)(A;;GA;;;"+user.Value+")");
    var bytes=new byte[descriptor.DiscretionaryAcl.BinaryLength];descriptor.DiscretionaryAcl.GetBinaryForm(bytes,0);
    acl=Marshal.AllocHGlobal(bytes.Length);Marshal.Copy(bytes,0,acl,bytes.Length);Marshal.WriteIntPtr(data,acl);
    if(!SetTokenInformation(linked,6,data,IntPtr.Size))throw new Exception("Set child token default DACL: "+Marshal.GetLastWin32Error());
    kind="LUA-restricted current-user token, medium integrity; no linked token available";
   }
   if(!GetTokenInformation(linked,20,data,4,out needed)||Marshal.ReadInt32(data)!=0)throw new Exception("Launcher token is elevated");
   var vars=new System.Collections.Generic.SortedDictionary<string,string>(StringComparer.OrdinalIgnoreCase);
   foreach(System.Collections.DictionaryEntry e in Environment.GetEnvironmentVariables())vars[(string)e.Key]=(string)e.Value;
   var block=new StringBuilder();foreach(var e in vars)block.Append(e.Key).Append('=').Append(e.Value).Append('\0');block.Append('\0');env=Marshal.StringToHGlobalUni(block.ToString());
   if(!CreateProcessAsUser(linked,app,cmd,IntPtr.Zero,IntPtr.Zero,false,0x08000400,env,cwd,ref si,out pi))throw new Exception("CreateProcessAsUser: "+Marshal.GetLastWin32Error());
   return kind;
  }finally{Marshal.FreeHGlobal(data);if(acl!=IntPtr.Zero)Marshal.FreeHGlobal(acl);if(env!=IntPtr.Zero)Marshal.FreeHGlobal(env);if(sid!=IntPtr.Zero)LocalFree(sid);if(linked!=IntPtr.Zero)CloseHandle(linked);if(token!=IntPtr.Zero)CloseHandle(token);}
 }
 [DllImport("kernel32.dll")]public static extern bool GetExitCodeProcess(IntPtr p,out uint code);
 public static bool Running(int id){IntPtr p=OpenProcess(0x1000,false,id);if(p==IntPtr.Zero){if(Marshal.GetLastWin32Error()==87)return false;throw new Exception("Cannot inspect process "+id);}try{uint code;if(!GetExitCodeProcess(p,out code))throw new Exception("Cannot inspect exit code");return code==259;}finally{CloseHandle(p);}}
 public delegate bool EnumProc(IntPtr w,IntPtr p);
 [DllImport("user32.dll")]public static extern bool EnumDesktopWindows(IntPtr d,EnumProc c,IntPtr p);
 [DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr w,out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr w,StringBuilder b,int n);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr w,uint m,IntPtr a,IntPtr b);
 public static string InputName(){IntPtr h=OpenInputDesktop(0,false,1);if(h==IntPtr.Zero)throw new Exception("No input desktop");try{int n;var b=new StringBuilder(256);if(!GetUserObjectInformation(h,2,b,512,out n))throw new Exception("Desktop name unavailable");return b.ToString();}finally{CloseDesktop(h);}}
 public static IntPtr Window(IntPtr d,uint p,string expected){IntPtr found=IntPtr.Zero;EnumProc c=(w,z)=>{uint id;GetWindowThreadProcessId(w,out id);var title=new StringBuilder(256);GetWindowText(w,title,256);if(id==p && IsWindowVisible(w) && title.ToString()==expected)found=w;return true;};EnumDesktopWindows(d,c,IntPtr.Zero);return found;}
}
'@
$candidate=Get-Content -LiteralPath $CandidateReport -Raw | ConvertFrom-Json
if (-not $candidate.installation) {
 $installReceipt=Get-Content -LiteralPath (Join-Path $candidate.directory 'install-report.json') -Raw | ConvertFrom-Json -AsHashtable
 $expectedDirectory=[IO.Path]::GetFullPath((Join-Path $candidate.directory 'installed'))
 if ($installReceipt.status -ne 'passed' -or $installReceipt.exitCode -ne 0 -or $installReceipt.installerSha256 -ne $candidate.installer.sha256 -or [IO.Path]::GetFullPath($installReceipt.installDirectory) -ne $expectedDirectory) { throw 'Successful matching install receipt required' }
 $candidate | Add-Member -NotePropertyName installation -NotePropertyValue ([pscustomobject]@{status='passed';directory=$expectedDirectory;binarySha256=$candidate.expectedInstalledBinary.sha256})
 $CandidateReport=Join-Path $candidate.directory 'gui-candidate-resolved.json'
 $candidate | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $CandidateReport -Encoding UTF8
}
$binary=Join-Path $candidate.installation.directory 'tinixcut.exe'
$stream=[IO.File]::OpenRead($binary);$sha=[Security.Cryptography.SHA256]::Create()
try{$hash=[BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant()}finally{$stream.Dispose();$sha.Dispose()}
if($hash -ne $candidate.expectedInstalledBinary.sha256){throw 'Candidate hash mismatch'}
$existing=Get-CimInstance Win32_Process -Filter "Name='tinixcut.exe'" | Where-Object {$_.ExecutablePath -eq $binary -and [InstalledDesktop]::Running($_.ProcessId)}
if($existing -and -not $ResumeOwned){throw 'Candidate already running'}
if($ResumeOwned){
 $prior=Get-Content -LiteralPath (Join-Path $candidate.directory 'gui-preflight.json') -Raw | ConvertFrom-Json
 if(-not $existing -or $existing.ProcessId -ne $prior.processId -or $prior.binarySha256 -ne $hash){throw 'Cannot validate owned preflight process'}
 Copy-Item -LiteralPath (Join-Path $candidate.directory 'gui-preflight.json') -Destination (Join-Path $candidate.directory ('gui-preflight-failed-'+[DateTime]::UtcNow.ToString('yyyyMMddHHmmss')+'.json'))
 $name=$prior.desktop
}else{$name='TinixCutInstalled-'+[guid]::NewGuid().ToString('N')}
$profile=Join-Path ([IO.Path]::GetTempPath()) $name
$oldArgs=$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--mute-audio'
if($Mode -eq 'accessibility'){$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS+=' --force-renderer-accessibility'}
if($Mode -eq 'cdp'){
 if(Get-NetTCPConnection -State Listen -LocalPort 9455 -ErrorAction SilentlyContinue){throw 'CDP port already occupied'}
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS+=' --remote-debugging-port=9455 --enable-logging --log-file="'+(Join-Path $candidate.directory 'webview-diagnostic.log')+'"'
}
$old=$env:WEBVIEW2_USER_DATA_FOLDER;$env:WEBVIEW2_USER_DATA_FOLDER=$profile
$before=[InstalledDesktop]::InputName()
$desktop=if($ResumeOwned){[InstalledDesktop]::OpenDesktop($name,0,$false,0x01ff)}else{[InstalledDesktop]::CreateDesktop($name,[IntPtr]::Zero,[IntPtr]::Zero,0,0x01ff,[IntPtr]::Zero)}
if($desktop -eq [IntPtr]::Zero){throw 'Isolated desktop unavailable'}
$pi=New-Object InstalledDesktop+PI;$window=[IntPtr]::Zero
$report=[ordered]@{startedAt=[DateTime]::UtcNow.ToString('o');binary=$binary;binarySha256=$hash;desktop=$name;profile=$profile;inputBefore=$before;status='running';mode=$Mode;browserArguments=$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;scope='Exact installed release on isolated desktop; diagnostic flags recorded; no native input, desktop switch or audio output'}
$report.launchToken=if($StandardUser){'linked non-elevated token of current user'}else{'inherited caller token'}
function Save-Probe([string]$Stage){$report.stage=$Stage;$report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $candidate.directory ('gui-'+$Mode+'-active.json')) -Encoding UTF8;Write-Output ('Installed GUI: '+$Stage)}
Save-Probe 'launch'
try{
 $si=New-Object InstalledDesktop+SI;$si.cb=[Runtime.InteropServices.Marshal]::SizeOf($si);$si.desktop='winsta0\'+$name
 $cmd=New-Object Text.StringBuilder ('"'+$binary+'"')
 if($ResumeOwned){$pi.id=$prior.processId;$pi.process=[InstalledDesktop]::OpenProcess(0x00100000 -bor 0x1000,$false,$pi.id);if($pi.process -eq [IntPtr]::Zero){throw 'Owned process handle unavailable'}}
 elseif($StandardUser){$report.launchToken=[InstalledDesktop]::LaunchStandard($binary,$cmd,$candidate.installation.directory,[ref]$si,[ref]$pi)}
 elseif(-not [InstalledDesktop]::CreateProcess($binary,$cmd,[IntPtr]::Zero,[IntPtr]::Zero,$false,0x08000000,[IntPtr]::Zero,$candidate.installation.directory,[ref]$si,[ref]$pi)){throw 'Candidate launch failed'}
 $report.processId=$pi.id
 Save-Probe 'launched'
 for($n=0;$n -lt 60;$n++){$window=[InstalledDesktop]::Window($desktop,$pi.id,$candidate.productName);if($window -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 250}
 if($window -eq [IntPtr]::Zero){throw 'No window observed on isolated desktop'}
 $title=New-Object Text.StringBuilder 256;[void][InstalledDesktop]::GetWindowText($window,$title,256)
 if($title.ToString() -ne $candidate.productName){throw 'Window identity mismatch'}
 $report.window=[ordered]@{handle=$window.ToInt64();title=$title.ToString();isolated=$true}
 Save-Probe 'window-observed'
 Start-Sleep -Seconds 3
 $report.webViews=@(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {$_.CommandLine -like "*$profile*"} | ForEach-Object {[ordered]@{id=$_.ProcessId;parentId=$_.ParentProcessId}})
 if($report.webViews.Count -eq 0){throw 'No profile-owned WebView observed'}
 Save-Probe 'webview-observed'
 $report.readOnlyUIA=[ordered]@{health='unverified';names=@()}
 if($Mode -ne 'cdp'){try{
  Add-Type -AssemblyName UIAutomationClient
  $root=[Windows.Automation.AutomationElement]::FromHandle($window)
  $nodes=$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
  $names=@();for($n=0;$n -lt [Math]::Min(500,$nodes.Count);$n++){$text=$nodes.Item($n).Current.Name;if($text){$names+=@($text)}}
  $report.readOnlyUIA.names=$names
  if(($names -join ' ') -match 'Core Healthy'){$report.readOnlyUIA.health='healthy badge observed'}
 }catch{$report.readOnlyUIA.error=$_.Exception.Message}}
 if($Mode -eq 'cdp'){
  Save-Probe 'cdp-readiness'
  $ready=$false
  for($n=0;$n -lt 40;$n++){try{$pages=Invoke-RestMethod http://127.0.0.1:9455/json/list -TimeoutSec 1;if($pages){$ready=$true;break}}catch{};Start-Sleep -Milliseconds 250}
  if(-not $ready){throw 'Installed CDP unavailable'}
  $listeners=@(Get-NetTCPConnection -State Listen -LocalPort 9455)
  foreach($listener in $listeners){
   if($listener.LocalAddress -notin @('127.0.0.1','::1')){throw 'CDP is not loopback-only'}
   $owner=Get-CimInstance Win32_Process -Filter ('ProcessId='+$listener.OwningProcess)
   if($owner.CommandLine -notlike "*$profile*"){throw 'CDP owner does not match profile'}
   if($owner.CommandLine -notmatch '(^| )--mute-audio( |$)'){throw 'Runtime mute-audio flag was not applied'}
  }
  $report.runtimeMuteAudioVerified=$true
  $report.cdpListeners=@($listeners | ForEach-Object {[ordered]@{address=$_.LocalAddress;processId=$_.OwningProcess}})
  $nodeInfo=New-Object Diagnostics.ProcessStartInfo
  $nodeInfo.FileName=(Get-Command node.exe).Source;$nodeInfo.Arguments='scripts/qualification/background/installed-gui-preview.mjs'
  $nodeInfo.WorkingDirectory=$PWD.Path;$nodeInfo.UseShellExecute=$false;$nodeInfo.CreateNoWindow=$true
  $nodeInfo.EnvironmentVariables['TINIXCUT_CANDIDATE_REPORT']=(Resolve-Path -LiteralPath $CandidateReport).Path
  $nodeInfo.RedirectStandardOutput=$true;$nodeInfo.RedirectStandardError=$true
  $node=New-Object Diagnostics.Process;$node.StartInfo=$nodeInfo
  try{[void]$node.Start();$childOut=$node.StandardOutput.ReadToEndAsync();$childErr=$node.StandardError.ReadToEndAsync();if(-not $node.WaitForExit(180000)){throw 'Installed preview child timed out; no force-stop attempted'};$report.child=[ordered]@{exitCode=$node.ExitCode;stdout=$childOut.Result;stderr=$childErr.Result};if($node.ExitCode -ne 0){throw 'Installed preview child failed'}}finally{$node.Dispose()}
  $report.previewReport=Join-Path $candidate.directory 'gui-preview-latest.json'
 }
 $report.inputAfter=[InstalledDesktop]::InputName()
 if($report.inputAfter -ne $before){throw 'Input desktop changed'}
 $report.status='passed'
}catch{$report.status='failed';$report.error=$_.Exception.Message;throw}
finally{
 if($window -ne [IntPtr]::Zero){$report.closeRequested=[InstalledDesktop]::PostMessage($window,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)}
 if($Mode -eq 'cdp' -and $report.cdpListeners){node scripts/qualification/background/installed-cdp-close.mjs}
 [uint32]$code=259
 if($pi.process -ne [IntPtr]::Zero){for($n=0;$n -lt 20;$n++){[void][InstalledDesktop]::GetExitCodeProcess($pi.process,[ref]$code);if($code -ne 259){break};Start-Sleep -Milliseconds 250}}
 $report.appExited=$code -ne 259
 $report.appExitCode=$code
 try{for($n=0;$n -lt 20;$n++){
  $report.remainingWebViews=@(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {$_.CommandLine -like "*$profile*" -and [InstalledDesktop]::Running($_.ProcessId)} | ForEach-Object {$_.ProcessId})
  if($report.remainingWebViews.Count -eq 0){break};Start-Sleep -Milliseconds 250
 }}catch{$report.cleanupInspectionError=$_.Exception.Message;$report.status='failed'}
 $report.profileRetained=Test-Path -LiteralPath $profile
 $report.inputFinal=[InstalledDesktop]::InputName()
 Save-Probe 'cleanup-inspected'
 if(-not $report.appExited -or $report.remainingWebViews.Count -gt 0 -or $report.inputFinal -ne $before){$report.status='failed'}
 if($pi.process -ne [IntPtr]::Zero){[void][InstalledDesktop]::CloseHandle($pi.process)}
 if($pi.thread -ne [IntPtr]::Zero){[void][InstalledDesktop]::CloseHandle($pi.thread)}
 [void][InstalledDesktop]::CloseDesktop($desktop)
 if($null -eq $old){Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue}else{$env:WEBVIEW2_USER_DATA_FOLDER=$old}
 if($null -eq $oldArgs){Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue}else{$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$oldArgs}
 $report.finishedAt=[DateTime]::UtcNow.ToString('o');$outputName=if($Mode -eq 'vanilla'){'gui-preflight.json'}else{'gui-'+$Mode+'-'+[DateTime]::UtcNow.ToString('yyyyMMddHHmmss')+'.json'};$report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $candidate.directory $outputName) -Encoding UTF8
 Write-Output ('Installed GUI final status: '+$report.status+'; '+$report.error)
 if($report.status -ne 'passed'){exit 1}
}
