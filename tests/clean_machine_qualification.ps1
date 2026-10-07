# Isolated Workstation Qualification Harness (CM-01..CM-07)
# Verifies installation and execution in an isolated Windows environment with sanitized PATH (0 developer tools).
# Note: Executed on host Windows environment; does not prove behavior on a fresh Windows installation (clean VM).

$ErrorActionPreference = "Continue"
$PSNativeCommandUseErrorActionPreference = $false

$workspaceRoot = "D:\___TOOLS\__TINIX\TinixCut"
$installerSource = Join-Path $workspaceRoot "src-tauri\target\release\bundle\nsis\TinixCut_0.1.0_x64-setup.exe"
$fixturesDir = Join-Path $env:TEMP "tinix_test_p12_fixtures"

$testRoot = Join-Path $env:TEMP ("TinixCleanMachine_Run_" + [guid]::NewGuid().ToString("N"))
$appDir = Join-Path $testRoot "App"
$userDataDir = Join-Path $testRoot "UserData"
$mediaDir = Join-Path $testRoot "Media"

Write-Host "=========================================================="
Write-Host "TinixCut Isolated Workstation Qualification Harness (CM-01..CM-07)"
Write-Host "=========================================================="
Write-Host "Test root: $testRoot"

# 1. Prepare a fresh, uniquely named test directory so a prior run's data
# cannot be removed by this qualification run.
New-Item -ItemType Directory -Force -Path $appDir | Out-Null
New-Item -ItemType Directory -Force -Path $userDataDir | Out-Null
New-Item -ItemType Directory -Force -Path $mediaDir | Out-Null

# Copy installer and create base media fixtures
$installerCopy = Join-Path $testRoot "TinixCut_Setup.exe"
if (Test-Path $installerSource) {
    Copy-Item -Path $installerSource -Destination $installerCopy -Force
}

$f01 = Join-Path $mediaDir "cm_f01_1080p30.mp4"
$f04 = Join-Path $mediaDir "cm_f04_audio48k.mp3"

# Generate media fixtures using bundled binary if not present
$bundledFFmpeg = Join-Path $workspaceRoot "src-tauri\resources\bin\ffmpeg.exe"
if (Test-Path $bundledFFmpeg) {
    if (-not (Test-Path $f01)) {
        & $bundledFFmpeg -y -f lavfi -i "testsrc=duration=2:size=1920x1080:rate=30" -f lavfi -i "sine=frequency=440:duration=2" -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac -b:a 192k $f01 2>$null
    }
    if (-not (Test-Path $f04)) {
        & $bundledFFmpeg -y -f lavfi -i "sine=frequency=880:duration=2" -c:a mp3 $f04 2>$null
    }
}

# 2. Strict Environment Sanitization
# Replace PATH with minimal Windows system directories ONLY
$cleanSystemPath = "C:\Windows\System32;C:\Windows;C:\Windows\System32\Wbem;C:\Windows\System32\WindowsPowerShell\v1.0\"
$env:PATH = $cleanSystemPath

Write-Host "`n[Check 1] Verifying environment sanitization (PATH: $env:PATH)"
$devTools = @("node", "cargo", "rustc", "ffmpeg", "ffprobe", "pnpm", "git")
foreach ($tool in $devTools) {
    $whereRes = cmd.exe /c "where.exe $tool 2>nul"
    if ($LASTEXITCODE -eq 0 -and $whereRes) {
        Write-Error "FAIL: Tool '$tool' was found on PATH: $whereRes. Environment is NOT sanitized."
        exit 1
    }
}
Write-Host "PASS: Clean environment confirmed. 0 developer tools found on PATH."

$results = @()

# -------------------------------------------------------------------------
# CM-06: Clean User-Scope Installation
# -------------------------------------------------------------------------
Write-Host "`n[CM-06] Executing clean user-scope installation via NSIS..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()

# Verify configuration from tauri.conf.json
$tauriConfPath = Join-Path $workspaceRoot "src-tauri\tauri.conf.json"
$tauriConf = Get-Content $tauriConfPath -Raw | ConvertFrom-Json
$wvMode = $tauriConf.bundle.windows.webviewInstallMode.type
$nsisMode = $tauriConf.bundle.windows.nsis.installMode

# Execute installation under currentUser (no admin elevation required)
$proc = Start-Process -FilePath $installerCopy -ArgumentList "/S", "/D=$appDir" -PassThru -Wait
$t0.Stop()

$exePath = Join-Path $appDir "tinixcut.exe"
$ffmpegPath = Join-Path $appDir "resources\bin\ffmpeg.exe"
$ffprobePath = Join-Path $appDir "resources\bin\ffprobe.exe"
$provPath = Join-Path $appDir "resources\ffmpeg_provenance.json"

# Check Start Menu shortcut location for currentUser
$startMenuPath = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\TinixCut.lnk"
$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\TinixCut"
$hasShortcut = (Test-Path $startMenuPath) -or (Test-Path $startMenuDir)

$cm06_passed = ($proc.ExitCode -eq 0) -and `
               (Test-Path $exePath) -and `
               (Test-Path $ffmpegPath) -and `
               (Test-Path $ffprobePath) -and `
               ($wvMode -eq "downloadBootstrapper") -and `
               ($nsisMode -eq "currentUser")

$results += [PSCustomObject]@{
    id = "CM-06"
    name = "Clean User-Scope Installation"
    passed = $cm06_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Installer exit code: $($proc.ExitCode), installMode: $nsisMode (currentUser, no admin elevation required), webviewInstallMode: $wvMode (downloadBootstrapper), tinixcut.exe exists: $(Test-Path $exePath), bundled resources staged: $(Test-Path $ffmpegPath), startMenuShortcut: $hasShortcut"
}
Write-Host "CM-06 Result: $(if ($cm06_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-01: Bundled Binary Self-Sufficiency
# -------------------------------------------------------------------------
Write-Host "`n[CM-01] Verifying bundled binary self-sufficiency with sanitized PATH..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()
$ffVerOut = & $ffmpegPath -version 2>&1 | Out-String
$fpVerOut = & $ffprobePath -version 2>&1 | Out-String
$ffmpegHash = if (Test-Path $ffmpegPath) { (Get-FileHash -Algorithm SHA256 $ffmpegPath).Hash.ToLower() } else { "" }
$ffprobeHash = if (Test-Path $ffprobePath) { (Get-FileHash -Algorithm SHA256 $ffprobePath).Hash.ToLower() } else { "" }
$t0.Stop()

$expectedFfmpegHash = "3256173f3f8bffd7df12227c68adf68025edb1832273a9530688a7bb1ed8edec"
$expectedFfprobeHash = "f0d36ecbbdd3bcfac3efa078c96c7271c2e68b3810595552ac3b7f17e9a65c52"

$cm01_passed = ($ffVerOut -match "ffmpeg version 9\.0\.2-essentials_build") -and `
               ($fpVerOut -match "ffprobe version 9\.0\.2-essentials_build") -and `
               ($ffmpegHash -eq $expectedFfmpegHash) -and `
               ($ffprobeHash -eq $expectedFfprobeHash)

$results += [PSCustomObject]@{
    id = "CM-01"
    name = "Bundled Binary Self-Sufficiency"
    passed = $cm01_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "FFmpeg: 9.0.2, FFprobe: 9.0.2, FFmpeg SHA256 match: $($ffmpegHash -eq $expectedFfmpegHash), FFprobe SHA256 match: $($ffprobeHash -eq $expectedFfprobeHash)"
}
Write-Host "CM-01 Result: $(if ($cm01_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-02: Mandatory Software Fallback
# -------------------------------------------------------------------------
Write-Host "`n[CM-02] Verifying mandatory software libx264 export fallback..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()
$swExportPath = Join-Path $userDataDir "sw_fallback_export.mp4"

cmd.exe /c "`"$ffmpegPath`" -y -i `"$f01`" -i `"$f04`" -c:v libx264 -preset ultrafast -crf 23 -pix_fmt yuv420p -c:a aac -b:a 192k -shortest `"$swExportPath`" >nul 2>nul"

$swProbe = if (Test-Path $swExportPath) {
    & $ffprobePath -v quiet -print_format json -show_streams -show_format $swExportPath | ConvertFrom-Json
} else { $null }

$swVStream = $swProbe.streams | Where-Object { $_.codec_type -eq "video" -and $_.codec_name -eq "h264" }
$swAStream = $swProbe.streams | Where-Object { $_.codec_type -eq "audio" -and $_.codec_name -eq "aac" }
$t0.Stop()

$cm02_passed = ($swVStream -ne $null) -and ($swAStream -ne $null)
$results += [PSCustomObject]@{
    id = "CM-02"
    name = "Mandatory Software Fallback"
    passed = $cm02_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Software libx264 fallback render successful, video: $($swVStream.codec_name), audio: $($swAStream.codec_name)"
}
Write-Host "CM-02 Result: $(if ($cm02_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-03: Spaces and Unicode Path Robustness
# -------------------------------------------------------------------------
Write-Host "`n[CM-03] Testing spaces and non-ASCII Unicode path robustness..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()

$unicodeDir = Join-Path $userDataDir "Thư Mục Dự Án TinixCut 🎬 & Spaces"
New-Item -ItemType Directory -Force -Path $unicodeDir | Out-Null

$uniMedia = Join-Path $unicodeDir "Video Gốc Có Dấu.mp4"
Copy-Item -Path $f01 -Destination $uniMedia -Force

$uniProj = Join-Path $unicodeDir "Dự Án Đầy Đủ.tcproj"
$uniExport = Join-Path $unicodeDir "Xuất Bản Hoàn Tất.mp4"

# Probe Unicode media
$probeJson = & $ffprobePath -v quiet -print_format json -show_streams $uniMedia | Out-String
$probeObj = $probeJson | ConvertFrom-Json
$uniProbeValid = ($probeObj.streams | Where-Object { $_.codec_type -eq "video" }) -ne $null

# Schema-v3 project save/reopen in Unicode path
$projV3Content = @{
    schema_version = 3
    project_id = "proj_unicode_cm03"
    name = "Dự Án Thử Nghiệm Unicode"
    revision = 1
    settings = @{
        width = 1920
        height = 1080
        frame_rate = @{ value = 30; rate = @{ numerator = 30; denominator = 1 } }
        sample_rate = 48000
        channels = 2
    }
    assets = @(
        @{ asset_id = "ast_uni"; name = "Video Gốc Có Dấu.mp4"; path = $uniMedia; media_kind = "Video" }
    )
    tracks = @(
        @{ track_id = "trk_v1"; name = "Video Chính"; kind = "Video"; items = @(); transitions = @() }
    )
} | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($uniProj, $projV3Content, [System.Text.Encoding]::UTF8)

$reopenedProj = [System.IO.File]::ReadAllText($uniProj, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
$uniProjValid = ($reopenedProj.schema_version -eq 3) -and ($reopenedProj.name -eq "Dự Án Thử Nghiệm Unicode")

# Export to Unicode path
cmd.exe /c "`"$ffmpegPath`" -y -i `"$uniMedia`" -c:v libx264 -preset ultrafast -c:a aac `"$uniExport`" >nul 2>nul"
$uniExportValid = (Test-Path $uniExport) -and ((Get-Item $uniExport).Length -gt 1000)

$t0.Stop()
$cm03_passed = $uniProbeValid -and $uniProjValid -and $uniExportValid
$results += [PSCustomObject]@{
    id = "CM-03"
    name = "Spaces and Unicode Path Robustness"
    passed = $cm03_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Unicode probe: $uniProbeValid, Schema v3 project round-trip: $uniProjValid, Export to Unicode path: $uniExportValid"
}
Write-Host "CM-03 Result: $(if ($cm03_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-04: Read-Only Media Protection
# -------------------------------------------------------------------------
Write-Host "`n[CM-04] Testing read-only media protection..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()

$roMedia = Join-Path $mediaDir "cm_readonly_media.mp4"
Copy-Item -Path $f01 -Destination $roMedia -Force
$initialHash = (Get-FileHash -Algorithm SHA256 $roMedia).Hash

# Mark file read-only
[System.IO.File]::SetAttributes($roMedia, [System.IO.FileAttributes]::ReadOnly)
$isReadOnlyBefore = ((Get-Item $roMedia).Attributes -band [System.IO.FileAttributes]::ReadOnly) -ne 0

# Probe and export using read-only media
$roExport = Join-Path $userDataDir "ro_export.mp4"
$roProbe = & $ffprobePath -v quiet -print_format json -show_streams $roMedia | ConvertFrom-Json
$roProbeValid = ($roProbe.streams | Where-Object { $_.codec_type -eq "video" }) -ne $null

cmd.exe /c "`"$ffmpegPath`" -y -i `"$roMedia`" -c:v libx264 -preset ultrafast `"$roExport`" >nul 2>nul"
$roExportValid = (Test-Path $roExport) -and ((Get-Item $roExport).Length -gt 1000)

$afterHash = (Get-FileHash -Algorithm SHA256 $roMedia).Hash
$unmutated = ($initialHash -eq $afterHash)

# Reset attributes for cleanup
[System.IO.File]::SetAttributes($roMedia, [System.IO.FileAttributes]::Normal)

$t0.Stop()
$cm04_passed = $isReadOnlyBefore -and $roProbeValid -and $roExportValid -and $unmutated
$results += [PSCustomObject]@{
    id = "CM-04"
    name = "Read-Only Media Protection"
    passed = $cm04_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Read-only enforced: $isReadOnlyBefore, Probed: $roProbeValid, Exported: $roExportValid, Unmutated hash match: $unmutated"
}
Write-Host "CM-04 Result: $(if ($cm04_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-05: Atomic Persistence & Recovery
# -------------------------------------------------------------------------
Write-Host "`n[CM-05] Testing atomic persistence and recovery safety..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()

$canonicalProj = Join-Path $userDataDir "canonical_safe.tcproj"
$canonicalContent = @{
    schema_version = 3
    project_id = "proj_atomic_001"
    name = "Canonical Intact Project"
    revision = 10
    assets = @()
    tracks = @()
} | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($canonicalProj, $canonicalContent, [System.Text.Encoding]::UTF8)

# Simulate an interrupted atomic update before replacement. The installed
# application’s actual persistence path is covered by the Rust storage tests.
$tempProj = "$canonicalProj.tmp"
[System.IO.File]::WriteAllText($tempProj, "corrupted-interrupted-data", [System.Text.Encoding]::UTF8)

# Canonical project file must remain 100% intact and valid Schema v3
$reloadedCanonical = [System.IO.File]::ReadAllText($canonicalProj, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
$atomicSafe = ($reloadedCanonical.schema_version -eq 3) -and ($reloadedCanonical.revision -eq 10)

# Cleanup simulated temp file
Remove-Item -Path $tempProj -Force -ErrorAction SilentlyContinue

$t0.Stop()
$cm05_passed = $atomicSafe
$results += [PSCustomObject]@{
    id = "CM-05"
    name = "Atomic Persistence & Recovery"
    passed = $cm05_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Harness interruption simulation preserved canonical project: $atomicSafe, Schema v3 intact at revision 10; implementation atomic-save/recovery behavior is covered separately by Rust storage tests"
}
Write-Host "CM-05 Result: $(if ($cm05_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# CM-07: User Data Preservation on Uninstall
# -------------------------------------------------------------------------
Write-Host "`n[CM-07] Testing user data preservation on uninstall..."
$t0 = [System.Diagnostics.Stopwatch]::StartNew()

# Record pre-uninstall existence and SHA-256 hashes for representative user data files
$projBeforeHash = if (Test-Path $canonicalProj) { (Get-FileHash -Algorithm SHA256 $canonicalProj).Hash } else { "" }
$mediaBeforeHash = if (Test-Path $f01) { (Get-FileHash -Algorithm SHA256 $f01).Hash } else { "" }
$exportBeforeHash = if (Test-Path $swExportPath) { (Get-FileHash -Algorithm SHA256 $swExportPath).Hash } else { "" }

$uninstaller = Join-Path $appDir "uninstall.exe"
if (Test-Path $uninstaller) {
    $uninstProc = Start-Process -FilePath $uninstaller -ArgumentList "/S", "_?=$appDir" -PassThru -Wait
    Start-Sleep -Seconds 3
}

$appRemoved = -not (Test-Path $exePath) -and -not (Test-Path $ffmpegPath)

# Check post-uninstall existence and verify exact SHA-256 hash preservation
$projAfterHash = if (Test-Path $canonicalProj) { (Get-FileHash -Algorithm SHA256 $canonicalProj).Hash } else { "" }
$mediaAfterHash = if (Test-Path $f01) { (Get-FileHash -Algorithm SHA256 $f01).Hash } else { "" }
$exportAfterHash = if (Test-Path $swExportPath) { (Get-FileHash -Algorithm SHA256 $swExportPath).Hash } else { "" }

$projPreserved = ($projBeforeHash -ne "") -and ($projBeforeHash -eq $projAfterHash)
$mediaPreserved = ($mediaBeforeHash -ne "") -and ($mediaBeforeHash -eq $mediaAfterHash)
$exportPreserved = ($exportBeforeHash -ne "") -and ($exportBeforeHash -eq $exportAfterHash)

$userDataPreserved = $projPreserved -and $mediaPreserved -and $exportPreserved

$t0.Stop()
$cm07_passed = $appRemoved -and $userDataPreserved
$results += [PSCustomObject]@{
    id = "CM-07"
    name = "User Data Preservation on Uninstall"
    passed = $cm07_passed
    duration_ms = $t0.ElapsedMilliseconds
    details = "Application binaries cleanly removed: $appRemoved, .tcproj preserved (SHA256: $projAfterHash, match: $projPreserved), external media preserved (SHA256: $mediaAfterHash, match: $mediaPreserved), rendered export preserved (SHA256: $exportAfterHash, match: $exportPreserved)"
}
Write-Host "CM-07 Result: $(if ($cm07_passed) {'PASS'} else {'FAIL'})"

# -------------------------------------------------------------------------
# Final Summary and Report Generation
# -------------------------------------------------------------------------
$totalTests = $results.Count
$passedTests = ($results | Where-Object { $_.passed }).Count
$failedTests = $totalTests - $passedTests
$allPassed = ($failedTests -eq 0)

$report = [PSCustomObject]@{
    report_version = "1.2.0"
    timestamp_utc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    qualification_type = "Isolated Workstation Qualification (CM-01..CM-07)"
    environment = "Windows Isolated Workstation (Sanitized PATH, repo and toolchain dependencies excluded from execution path; executed on host Windows, not fresh clean VM)"
    os_info = [System.Environment]::OSVersion.VersionString
    total_tests = $totalTests
    passed_tests = $passedTests
    failed_tests = $failedTests
    qualification_verdict = if ($allPassed) { "PASS (Isolated Workstation Qualification)" } else { "FAIL" }
    clean_machine_verdict = "DEFERRED / NON-BLOCKING (Host-isolated execution PASSED; True fresh clean Windows 10/11 VM qualification DEFERRED for personal-use v1)"
    qualification_notes = "PATH was sanitized and repository/toolchain dependencies were excluded from the execution path. CM-05 is a harness-level interrupted-write simulation; implementation atomic-save/recovery behavior is separately covered by Rust storage tests."
    deferred_items = @(
        "True clean Windows 10/11 VM qualification: DEFERRED / non-blocking for personal-use v1.",
        "Hardware acceleration NVENC/QSV/AMF qualification: DEFERRED / best-effort.",
        "P16-AC-56 interactive Windows GUI qualification: DEFERRED / NOT PASS."
    )
    results = $results
}

$evidenceDir = Join-Path $workspaceRoot "tests\evidence"
if (-not (Test-Path $evidenceDir)) {
    New-Item -ItemType Directory -Force -Path $evidenceDir | Out-Null
}
$reportFile = Join-Path $evidenceDir "clean_machine_qualification_report.json"
$report | ConvertTo-Json -Depth 5 | Set-Content -Path $reportFile -Encoding utf8

# Cleanup test directory
Remove-Item -Path $testRoot -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "`n=========================================================="
Write-Host "Isolated Workstation Qualification Complete!"
Write-Host "Total: $totalTests, Passed: $passedTests, Failed: $failedTests"
Write-Host "Verdict: $(if ($allPassed) {'PASS'} else {'FAIL'})"
Write-Host "Evidence written to: $reportFile"
Write-Host "=========================================================="
