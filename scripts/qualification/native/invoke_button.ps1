param([string]$Name)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$proc=Get-Process tinixcut | Select-Object -First 1
$win=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$proc.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$cond=New-Object System.Windows.Automation.AndCondition((New-Object $C($A::NameProperty,$Name)),(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))
$b=$win.FindAll($T::Descendants,$cond)
if($b.Count -ne 1){ throw "expected 1 button '$Name', found $($b.Count)" }
$b[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); "Invoked $Name"
