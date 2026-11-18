$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'cdp-fallback-controls.ps1')
$root=Join-Path ([IO.Path]::GetTempPath()) ('tinixcut-fallback-test-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -ErrorAction Stop | Out-Null
$cache=Join-Path $root 'cache'
$catalog=Join-Path $root 'catalog'
New-Item -ItemType Directory -Path $cache,$catalog -ErrorAction Stop | Out-Null
[IO.File]::WriteAllText((Join-Path $cache 'control.txt'),'cache control')
[IO.File]::WriteAllText((Join-Path $catalog 'control.txt'),'catalog control')
$cacheBefore=Get-ControlInventory $cache
$catalogBefore=Get-ControlInventory $catalog
if((Get-ControlInventory $cache) -cne $cacheBefore -or (Get-ControlInventory $catalog) -cne $catalogBefore) {throw 'Unchanged controls did not match'}
[IO.File]::WriteAllText((Join-Path $cache 'control.txt'),'changed')
if((Get-ControlInventory $cache) -ceq $cacheBefore) {throw 'Modified cache control was not detected'}
Remove-Item -LiteralPath (Join-Path $cache 'control.txt') -ErrorAction Stop
if((Get-ControlInventory $cache) -ceq $cacheBefore) {throw 'Deleted cache control was not detected'}
[IO.File]::WriteAllText((Join-Path $catalog 'unexpected.txt'),'unexpected')
if((Get-ControlInventory $catalog) -ceq $catalogBefore) {throw 'Unexpected catalog file was not detected'}
Write-Output ('PASS: unchanged, modified/deleted cache and unexpected catalog entry; fixtures retained at '+$root)
