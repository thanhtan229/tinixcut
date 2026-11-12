Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst('Children',(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$wp=$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
if($wp.Current.WindowVisualState -eq 'Minimized'){ $wp.SetWindowVisualState('Normal'); Start-Sleep 1 }
"state " + $wp.Current.WindowVisualState
