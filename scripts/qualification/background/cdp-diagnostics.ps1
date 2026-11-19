param([string]$SessionPath, [string]$Label='after-parent-exit', [string]$OutputPath)
$ErrorActionPreference='Stop'
if(-not ('CdpReadOnlyProbe' -as [type])) {
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class CdpReadOnlyProbe {
 [StructLayout(LayoutKind.Sequential)] public struct UniqueProcess {public uint Id; public System.Runtime.InteropServices.ComTypes.FILETIME Start;}
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Owner {public UniqueProcess Process; [MarshalAs(UnmanagedType.ByValTStr,SizeConst=256)]public string Name; [MarshalAs(UnmanagedType.ByValTStr,SizeConst=64)] public string Service; public int Type; public uint Status,Session; [MarshalAs(UnmanagedType.Bool)]public bool Restartable;}
 [DllImport("rstrtmgr.dll",CharSet=CharSet.Unicode)] static extern int RmStartSession(out uint handle,uint flags,StringBuilder key);
 [DllImport("rstrtmgr.dll")] static extern int RmEndSession(uint handle);
 [DllImport("rstrtmgr.dll",CharSet=CharSet.Unicode)] static extern int RmRegisterResources(uint handle,uint files,string[] paths,uint apps,IntPtr processes,uint services,string[] serviceNames);
 [DllImport("rstrtmgr.dll")] static extern int RmGetList(uint handle,out uint needed,ref uint count,[In,Out]Owner[] apps,ref uint reasons);
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint id);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
 [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr handle);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool GetUserObjectInformation(IntPtr h,int index,StringBuilder text,int length,out int needed);
 public static string State(uint id) {
  IntPtr h=OpenProcess(0x1000,false,id);
  if(h==IntPtr.Zero)return Marshal.GetLastWin32Error()==87 ? "exited" : "unknown";
  try {uint code;if(!GetExitCodeProcess(h,out code))return "unknown";return code==259 ? "running" : "exited";}finally{CloseHandle(h);}
 }
 public static string InputDesktop() {
  IntPtr h=OpenInputDesktop(0,false,1);if(h==IntPtr.Zero)throw new System.ComponentModel.Win32Exception();
  try{int needed;var s=new StringBuilder(256);if(!GetUserObjectInformation(h,2,s,512,out needed))throw new System.ComponentModel.Win32Exception();return s.ToString();}finally{CloseDesktop(h);}
 }
 public static Owner[] Owners(string path) {
  uint h;int error=RmStartSession(out h,0,new StringBuilder(33));if(error!=0)throw new Exception("RmStartSession: "+error);
  try{
   error=RmRegisterResources(h,1,new[]{path},0,IntPtr.Zero,0,null);if(error!=0)throw new Exception("RmRegisterResources: "+error);
   uint needed=0,count=0,reasons=0;error=RmGetList(h,out needed,ref count,null,ref reasons);if(error==0)return new Owner[0];if(error!=234)throw new Exception("RmGetList: "+error);
   if(needed>64)throw new Exception("Lock owner count exceeds diagnostic bound");
   count=needed;var owners=new Owner[count];error=RmGetList(h,out needed,ref count,owners,ref reasons);if(error!=0)throw new Exception("RmGetList: "+error);return owners;
  }finally{RmEndSession(h);}
 }
}
'@
}

function Get-CdpSnapshot($Session, [string]$SnapshotLabel) {
 $tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
 $binary=Join-Path $PWD.Path 'src-tauri\target-qualification\debug\tinixcut.exe'
 $profile=$null
 if($Session -and $Session.desktop){
  if($Session.desktop -notmatch '^TinixCutCDP-[0-9a-f]{32}$'){throw 'Invalid diagnostic desktop identifier'}
  $profile=Join-Path $tempRoot $Session.desktop
 }
 $processes=@()
 $retainedPattern=[regex]::Escape((Join-Path $tempRoot 'TinixCutCDP-'))+'[0-9a-f]{32}(?=\\|")'
 foreach($p in @(Get-CimInstance Win32_Process -OperationTimeoutSec 5)) {
  $retainedWebView=$p.Name -eq 'msedgewebview2.exe' -and $p.CommandLine -match $retainedPattern
  if(($profile -and $p.CommandLine -like "*$profile*") -or $p.ExecutablePath -eq $binary -or $retainedWebView){
   $processes+=@([ordered]@{processId=[int]$p.ProcessId;parentId=[int]$p.ParentProcessId;name=[string]$p.Name;createdAt=([DateTime]$p.CreationDate).ToUniversalTime().ToString('o');state=[CdpReadOnlyProbe]::State($p.ProcessId);qualificationApp=($p.ExecutablePath -eq $binary)})
  }
  $p.Dispose()
 }
 $appState=if($Session -and $Session.processId){[CdpReadOnlyProbe]::State([uint32]$Session.processId)}elseif($Session){'unknown'}else{'not-started'}
 $listeners=@(Get-NetTCPConnection -State Listen -LocalPort 9445 -ErrorAction SilentlyContinue | ForEach-Object {[ordered]@{address=[string]$_.LocalAddress;processId=[int]$_.OwningProcess}})
 $locks=@()
 if($profile){
  foreach($relative in @('EBWebView\lockfile','EBWebView\Default\Extension Rules\000003.log')){
   $file=Join-Path $profile $relative
   $entry=[ordered]@{relativePath=$relative;exists=(Test-Path -LiteralPath $file);owners=@()}
   if($entry.exists){
    try{
     $entry.owners=@([CdpReadOnlyProbe]::Owners($file) | ForEach-Object {
      $stamp=([int64]$_.Process.Start.dwHighDateTime -shl 32) -bor [uint32]$_.Process.Start.dwLowDateTime
      [ordered]@{processId=[int]$_.Process.Id;createdAt=[DateTime]::FromFileTimeUtc($stamp).ToString('o');app=[string]$_.Name;state=[CdpReadOnlyProbe]::State($_.Process.Id)}
     })
    }catch{$entry.error=$_.Exception.Message}
   }
   $locks+=@($entry)
  }
 }
 $inputName=[CdpReadOnlyProbe]::InputDesktop()
 $expectedInput=if($Session -and $Session.inputDesktop){$Session.inputDesktop}else{'Default'}
 $unresolvedLocks=@($locks | Where-Object {$_.error -or @($_.owners | Where-Object {$_.state -ne 'exited'}).Count -gt 0})
 $runtimeSafe=(($appState -eq 'exited' -or $appState -eq 'not-started') -and @($processes | Where-Object {$_.state -ne 'exited'}).Count -eq 0 -and $listeners.Count -eq 0 -and $inputName -eq $expectedInput -and $unresolvedLocks.Count -eq 0)
 return [ordered]@{label=$SnapshotLabel;at=[DateTime]::UtcNow.ToString('o');profile=$profile;appState=$appState;processes=$processes;listeners=$listeners;inputDesktop=$inputName;inputUnchanged=($inputName -eq $expectedInput);locks=$locks;runtimeSafe=$runtimeSafe}
}

function Get-CdpTempBudget {
 $tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
 $roots=@(Get-ChildItem -LiteralPath $tempRoot -Directory -Filter 'TinixCutCDP-*' | Where-Object Name -Match '^TinixCutCDP-[0-9a-f]{32}$')
 $fixtures=Join-Path $tempRoot 'tinixcut-cdp-fixtures'
 if(Test-Path -LiteralPath $fixtures){$roots+=@(Get-ChildItem -LiteralPath $fixtures -Directory | Where-Object Name -Match '^TinixCutCDP-[0-9a-f]{32}$')}
 [long]$bytes=0;$entries=@();$errors=@()
 foreach($root in $roots){
  [long]$size=0
  try{
   if($root.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse point in diagnostic root'}
   foreach($file in @(Get-ChildItem -LiteralPath $root.FullName -Recurse -Force -File -ErrorAction Stop)){$size+=$file.Length}
  }catch{$errors+=@([string]$_.Exception.Message)}
  $bytes+=$size;$entries+=@([ordered]@{name=[string]$root.Name;path=[string]$root.FullName;bytes=$size})
 }
 $count=@($roots.Name | Sort-Object -Unique).Count
 return [ordered]@{at=[DateTime]::UtcNow.ToString('o');bytes=$bytes;retainedRuns=$count;maxBytes=1GB;reserveBytes=128MB;maxRetainedRuns=64;errors=$errors;allowed=($errors.Count -eq 0 -and $bytes+128MB -le 1GB -and $count -lt 64);entries=$entries}
}

if($OutputPath){
 $context=if($SessionPath){Get-Content -LiteralPath $SessionPath -Raw | ConvertFrom-Json}else{$null}
 $snapshot=Get-CdpSnapshot $context $Label
 $snapshot.budget=Get-CdpTempBudget
 $snapshot | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
}
