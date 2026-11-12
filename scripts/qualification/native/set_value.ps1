# Focus a named editable field and set its value through UIA ValuePattern; prints readback.
param([string]$Name, [string]$Value)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$f=@($w.FindAll($T::Descendants,(New-Object $C($A::NameProperty,$Name))) | Where-Object { $_.GetSupportedPatterns() -contains [System.Windows.Automation.ValuePattern]::Pattern })
if($f.Count -ne 1){ throw "expected 1 editable '$Name', found $($f.Count)" }
$f[0].SetFocus(); Start-Sleep -Milliseconds 150
$vp=$f[0].GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern); $vp.SetValue($Value); Start-Sleep -Milliseconds 200
"$Name = $($vp.Current.Value)"
