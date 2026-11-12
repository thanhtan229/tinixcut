# Toggle a uniquely named toggle button (aria-pressed) through UIA TogglePattern; prints state.
param([string]$Name)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$b=@($w.FindAll($T::Descendants,(New-Object $C($A::NameProperty,$Name))) | Where-Object { $_.GetSupportedPatterns() -contains [System.Windows.Automation.TogglePattern]::Pattern })
if($b.Count -ne 1){ throw "expected 1 toggle '$Name', found $($b.Count)" }
$tp=$b[0].GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern); $tp.Toggle(); Start-Sleep -Milliseconds 400
"$Name -> $($tp.Current.ToggleState)"
