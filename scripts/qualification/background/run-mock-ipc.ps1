param([string]$TestFilter='mock_tauri_ipc_')
$ErrorActionPreference = 'Stop'
$sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$mt = Get-ChildItem -Path $sdkRoot -Filter mt.exe -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\mt\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $mt) { throw 'Windows SDK mt.exe is required for the test manifest.' }
$messages = & cargo test --manifest-path src-tauri/Cargo.toml --lib --features mock-ipc-tests --no-run --message-format=json
if ($LASTEXITCODE -ne 0) { throw 'Cargo test build failed.' }
$exe = $messages | ForEach-Object { try { $_ | ConvertFrom-Json } catch {} } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.profile.test -eq $true -and $_.target.name -eq 'tinixcut_lib' -and $_.executable } | Select-Object -Last 1 -ExpandProperty executable
if (-not $exe) { throw 'Could not locate Cargo test executable.' }
$dir = Join-Path ([IO.Path]::GetTempPath()) ('tinixcut-mock-ipc-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $dir | Out-Null
try {
    $copy = Join-Path $dir 'mock-ipc.exe'
    Copy-Item -LiteralPath $exe -Destination $copy
    $manifest = Join-Path $PWD.Path 'scripts\qualification\background\common-controls.manifest'
    & $mt.FullName -nologo -manifest $manifest "-outputresource:$copy;#1"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to embed test manifest.' }
    & $copy $TestFilter --nocapture
    if ($LASTEXITCODE -ne 0) { throw "Mock IPC test failed: exit $LASTEXITCODE" }
} finally {
    Remove-Item -LiteralPath $dir -Recurse -Force
}
