# Two Go-to-frame submissions back to back: the second supersedes the first before it settles.
param([int]$First, [int]$Second)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$f=$w.FindFirst($T::Descendants,(New-Object $C($A::AutomationIdProperty,'preview-frame-seek')))
$form=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($f)
$go=$form.FindFirst($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::NameProperty,'Go')),(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))))
$vp=$f.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern); $ip=$go.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
$f.SetFocus(); $vp.SetValue([string]$First); $ip.Invoke(); $f.SetFocus(); $vp.SetValue([string]$Second); $ip.Invoke(); "seek2 $First -> $Second"
