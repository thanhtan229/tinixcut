param([int]$Frame)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$proc=Get-Process tinixcut | Select-Object -First 1
$win=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$proc.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$ws=$win.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Current.WindowVisualState.ToString()
$seekField=$win.FindFirst($T::Descendants,(New-Object $C($A::AutomationIdProperty,'preview-frame-seek')))
$form=[System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($seekField)
$go=$form.FindFirst($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::NameProperty,'Go')),(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))))
$vp=$seekField.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
$seekField.SetFocus(); Start-Sleep -Milliseconds 250; $vp.SetValue([string]$Frame); Start-Sleep -Milliseconds 150; $rb=$vp.Current.Value
$t=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
if($rb -eq [string]$Frame){ $go.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
$t2=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
@{readback=$rb; invokeEpochMs=$t; invokeReturnedEpochMs=$t2; goFound=($go -ne $null); windowState=$ws} | ConvertTo-Json -Compress
