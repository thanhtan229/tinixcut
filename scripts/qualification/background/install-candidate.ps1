param([Parameter(Mandatory=$true)][string]$CandidateReport, [switch]$VerifyExisting)
$ErrorActionPreference='Stop'
$candidate=Get-Content -LiteralPath $CandidateReport -Raw | ConvertFrom-Json
if($candidate.status -ne 'built' -or $candidate.productName -notmatch '^TinixCut Candidate [0-9-]+$' -or $candidate.identifier -notmatch '^com\.tinixcut\.candidate[0-9]+$'){throw 'Not an isolated built candidate'}
function Hash-File([string]$Path) {
 $stream=[IO.File]::OpenRead($Path);$sha=[Security.Cryptography.SHA256]::Create()
 try{return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant()}finally{$stream.Dispose();$sha.Dispose()}
}
function Read-Registry([string]$Name) {
 $result=[ordered]@{}
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry64,[Microsoft.Win32.RegistryView]::Registry32)){
  $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,$view)
  try {
   $key=$root.OpenSubKey($Name)
   if($null -eq $key){$result[$view.ToString()]=$null;continue}
   try{$values=[ordered]@{};foreach($entry in @($key.GetValueNames() | Sort-Object)){$values[$entry]=$key.GetValue($entry)};$result[$view.ToString()]=$values}finally{$key.Dispose()}
  }finally{$root.Dispose()}
 }
 return $result
}
function Production-Snapshot {
 $files=@()
 foreach($base in @($env:APPDATA,$env:LOCALAPPDATA)){
  $settings=Join-Path $base 'com.tinixcut.editor/settings.json'
  $files+=@([ordered]@{path=$settings;exists=(Test-Path -LiteralPath $settings);sha256=$(if(Test-Path -LiteralPath $settings){Hash-File $settings}else{$null})})
 }
 return [ordered]@{uninstall=(Read-Registry 'Software\Microsoft\Windows\CurrentVersion\Uninstall\TinixCut');location=(Read-Registry 'Software\tinixcut\TinixCut');settings=$files}
}
$directory=[IO.Path]::GetFullPath($candidate.directory)
$install=[IO.Path]::GetFullPath((Join-Path $directory 'installed'))
if([IO.Path]::GetDirectoryName($install) -ne $directory -or ((Test-Path -LiteralPath $install) -and -not $VerifyExisting)){throw 'Candidate install directory is not a new child of its artifact root'}
if((Hash-File $candidate.installer.path) -ne $candidate.installer.sha256){throw 'Installer hash changed'}
$candidateKey='Software\Microsoft\Windows\CurrentVersion\Uninstall\'+$candidate.productName
$registered=Read-Registry $candidateKey
if(@($registered.Values | Where-Object {$null -ne $_}).Count -and -not $VerifyExisting){throw 'Candidate already registered; no automatic reinstall or previous-version removal allowed'}
# Refuse any runtime bootstrap/download: the installed runtime must already be registered.
$runtimeVersions=@()
foreach($hive in @([Microsoft.Win32.RegistryHive]::LocalMachine,[Microsoft.Win32.RegistryHive]::CurrentUser)){
 foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)){
  $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
  try{$key=$root.OpenSubKey('SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}');if($key){try{$version=$key.GetValue('pv');if($version){$runtimeVersions+=@([string]$version)}}finally{$key.Dispose()}}}finally{$root.Dispose()}
 }
}
if($runtimeVersions.Count -eq 0){throw 'No registered WebView2 runtime; do not download/install a runtime in this qualification step'}
$receipt=[ordered]@{startedAt=[DateTime]::UtcNow.ToString('o');status='running';installerSha256=$candidate.installer.sha256;installDirectory=$install;productName=$candidate.productName;identifier=$candidate.identifier;registeredWebViewVersions=$runtimeVersions;productionBefore=(Production-Snapshot);scope='Isolated-identity release silent install/resource and CLI readiness only; GUI/upgrade/production identity unqualified'}
$receiptPath=Join-Path $directory $(if($VerifyExisting){'install-verification-report.json'}else{'install-report.json'})
try {
 if($VerifyExisting){
  $priorPath=Join-Path $directory 'install-first-check-failed.json'
  if(-not(Test-Path -LiteralPath $priorPath)){$priorPath=Join-Path $directory 'install-report.json'}
  $prior=Get-Content -LiteralPath $priorPath -Raw | ConvertFrom-Json -AsHashtable
  if($prior.installerSha256 -ne $candidate.installer.sha256 -or $prior.installDirectory -ne $install){throw 'Existing install provenance mismatch'}
  if($prior.exitCode -eq 0){$receipt.recordedInstallerExitCode=0;$receipt.provenance=$priorPath}
  elseif($candidate.installation.status -eq 'passed' -and $candidate.installation.directory -eq $install -and $candidate.installation.binarySha256 -eq $candidate.expectedInstalledBinary.sha256){
   # An earlier verifier could overwrite install-report before it inspected
   # anything. Keep that failure; use the successful certificate recorded by
   # the caller after the original installation, never fabricate its receipt.
   $receipt.provenance='candidate.installation success certificate; original receipt overwritten by earlier verifier'
   $receipt.originalReceiptUnavailable=$true
  }else{throw 'No successful existing installation provenance'}
  $receipt.productionBefore=$prior.productionBefore;$receipt.verificationOnly=$true
 }else{
 $process=Start-Process -FilePath $candidate.installer.path -ArgumentList @('/S','/NS',('/D='+$install)) -WindowStyle Hidden -PassThru
 try{
  if(-not $process.WaitForExit(120000)){
   $receipt.timedOut=$true;$receipt.installerProcessId=$process.Id
   $process.Kill();[void]$process.WaitForExit(5000)
   throw ('Candidate installer exceeded timeout; stopped owned PID '+$process.Id)
  }
  $process.Refresh();$receipt.exitCode=$process.ExitCode
 }finally{$process.Dispose()}
 }
 if(-not $VerifyExisting -and $receipt.exitCode -ne 0){throw ('Candidate installer failed: '+$receipt.exitCode)}
 $installedBinary=Join-Path $install 'tinixcut.exe'
 $receipt.binarySha256=Hash-File $installedBinary
 if($receipt.binarySha256 -ne $candidate.expectedInstalledBinary.sha256){throw 'Installed binary differs from frozen expected NSIS payload'}
 $receipt.resources=@()
 foreach($relative in @('resources/bin/ffmpeg.exe','resources/bin/ffprobe.exe','resources/ffmpeg_provenance.json')){
  $file=Join-Path $install $relative
  $name=[IO.Path]::GetFileName($file)
  $expected=@($candidate.artifacts | Where-Object name -eq $name)[0]
  $actual=Hash-File $file
  if($actual -ne $expected.sha256){throw ('Installed resource mismatch: '+$name)}
  $receipt.resources+=@([ordered]@{path=$file;sha256=$actual})
 }
 $receipt.candidateRegistry=Read-Registry $candidateKey
 $registeredLocations=@($receipt.candidateRegistry.Values | Where-Object {$null -ne $_} | ForEach-Object {$_.InstallLocation.Trim('"')})
 if($registeredLocations.Count -eq 0 -or @($registeredLocations | Where-Object {$_ -ne $install}).Count){throw 'Candidate registry points outside dedicated install directory'}
 $start=New-Object Diagnostics.ProcessStartInfo;$start.FileName=$installedBinary;$start.Arguments='--help';$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
 $cli=New-Object Diagnostics.Process;$cli.StartInfo=$start
 try{[void]$cli.Start();$out=$cli.StandardOutput.ReadToEndAsync();$err=$cli.StandardError.ReadToEndAsync();if(-not $cli.WaitForExit(15000)){$cli.Kill();throw 'Installed CLI help timeout'};$receipt.cli=[ordered]@{exitCode=$cli.ExitCode;stdout=[string]$out.Result;stderr=[string]$err.Result}}finally{$cli.Dispose()}
 if($receipt.cli.exitCode -ne 0 -or $receipt.cli.stdout -notmatch 'TinixCut'){throw 'Installed CLI readiness failed'}
 $receipt.productionAfter=Production-Snapshot
 $receipt.productionUnchanged=(($receipt.productionBefore | ConvertTo-Json -Depth 10 -Compress) -ceq ($receipt.productionAfter | ConvertTo-Json -Depth 10 -Compress))
 if(-not $receipt.productionUnchanged){throw 'Production registry/settings changed during install; investigate before acceptance'}
 $receipt.status='passed'
 Write-Output ('PASS: isolated candidate installed and hash/resource/CLI ready at '+$install)
}catch{$receipt.status='failed';$receipt.error=$_.Exception.Message;throw}
finally{$receipt.finishedAt=[DateTime]::UtcNow.ToString('o');$receipt | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $receiptPath -Encoding UTF8}
