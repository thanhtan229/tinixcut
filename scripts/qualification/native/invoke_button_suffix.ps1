param([string]$Suffix, [int]$MaxLength = 12)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$btns=$w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))
$m=@($btns | Where-Object { $n=$_.Current.Name.Trim(); $n.EndsWith(" $Suffix") -and $n.Length -le $MaxLength })
if($m.Count -ne 1){ throw "expected 1 button ending with '$Suffix', found $($m.Count)" }
$m[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); "Invoked $($m[0].Current.Name)"
