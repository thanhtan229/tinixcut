param([ValidateSet('play','pause','step','select')][string]$Action, [string]$ClipName = '')
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
function Buttons { $w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button))) }
switch ($Action) {
  'play'  { $b=@(Buttons | Where-Object { $_.Current.Name.Trim() -match '^\S+ Play$' }) }
  'pause' { $b=@(Buttons | Where-Object { $_.Current.Name.Trim() -match '^\S+ Pause$' }) }
  'step'  { $s=[string][char]0x25B6; $b=@(Buttons | Where-Object { $_.Current.Name.Trim() -eq $s }) }
  'select' {
    $g=@($w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Group))) | Where-Object { $_.Current.Name.StartsWith("$ClipName,") })
    if($g.Count -ne 1){ throw "expected 1 clip '$ClipName', found $($g.Count)" }
    # A focused clip selects on Enter (keyboard path). Only send the key when TinixCut
    # owns the foreground window, so keystrokes can never reach another application.
    $g[0].SetFocus(); Start-Sleep -Milliseconds 150
    Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);'
    $fgPid=[uint32]0; [void][W.U]::GetWindowThreadProcessId([W.U]::GetForegroundWindow(), [ref]$fgPid)
    if($fgPid -ne [uint32]$p.Id){ throw "TinixCut is not the foreground window (foreground pid $fgPid); Enter not sent" }
    Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); "selected $ClipName"; return
  }
}
if($b.Count -ne 1){ throw "expected 1 '$Action' button, found $($b.Count)" }
$b[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); "invoked $Action"
