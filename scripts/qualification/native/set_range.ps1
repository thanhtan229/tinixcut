# Set a named slider (UIA RangeValue) to a value and read it back.
param([string]$Name, [double]$Value)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$s=@($w.FindAll($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Slider)),(New-Object $C($A::NameProperty,$Name))))))
if($s.Count -ne 1){ throw "expected 1 slider '$Name', found $($s.Count)" }
$rv=$s[0].GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern); $rv.SetValue($Value); Start-Sleep -Milliseconds 300
"$Name = $($rv.Current.Value)"
