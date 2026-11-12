param([ValidateSet('On','Off')][string]$Want)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$cb=$w.FindFirst($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::CheckBox)),(New-Object $C($A::NameProperty,'Use seek-optimized proxies for video preview')))))
if(-not $cb){ throw 'proxy checkbox not found (is Media cache open?)' }
$tp=$cb.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
if($tp.Current.ToggleState.ToString() -ne $Want){ $tp.Toggle(); Start-Sleep -Milliseconds 800 }
"proxy checkbox " + $tp.Current.ToggleState
