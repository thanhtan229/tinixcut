function Get-ControlInventory([string]$Path) {
 if (-not (Test-Path -LiteralPath $Path -PathType Container)) { throw "Control directory missing: $Path" }
 $items=@(Get-ChildItem -LiteralPath $Path -Force -Recurse | Sort-Object FullName)
 return (($items | ForEach-Object {
  $name=$_.FullName.Substring($Path.Length).TrimStart('\')
  if($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point in control directory: $name" }
  if($_.PSIsContainer) { 'D:'+ $name } else { 'F:'+ $name + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
 }) -join "`n")
}
