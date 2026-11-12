param([string]$Path)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]
$C=[System.Windows.Automation.PropertyCondition]
$proc=Get-Process tinixcut | Select-Object -First 1
$win=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$proc.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$btns=$win.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))
$open=$null; foreach($b in $btns){ $n=$b.Current.Name.Trim(); if($n.EndsWith(' Open') -and $n.Length -le 8){ $open=$b; break } }
if(-not $open){ throw "Open button not found" }
"Invoking: $($open.Current.Name)"
$open.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
$dlg=$null
for($i=0;$i -lt 40 -and -not $dlg;$i++){ Start-Sleep -Milliseconds 250
  $dlg=$win.FindFirst($T::Children,(New-Object $C($A::ClassNameProperty,'#32770'))) }
if(-not $dlg){ throw "File dialog did not appear" }
"Dialog: $($dlg.Current.Name)"
$edit=$dlg.FindFirst($T::Descendants,(New-Object $C($A::AutomationIdProperty,'1148')))
$edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Path)
"Filename readback: $($edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value)"
$ok=$dlg.FindFirst($T::Children,(New-Object $C($A::AutomationIdProperty,'1')))
$ok.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
"Submitted"
