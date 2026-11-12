# Invoke the -Index-th (0-based, document order) button named -Action, e.g. the Preview
# button of one media-library row. Chromium's accessibility tree omits the row containers,
# so callers resolve the index in-page (see asset_button_index in g50 driver / README).
param([string]$Action, [int]$Index)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$b=@($w.FindAll($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)),(New-Object $C($A::NameProperty,$Action))))))
if($Index -ge $b.Count){ throw "only $($b.Count) '$Action' buttons" }
$b[$Index].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); "invoked $Action #$Index of $($b.Count)"
