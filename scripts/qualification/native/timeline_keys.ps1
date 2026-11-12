# Focus a timeline element and send keys to it (keyboard editing path).
#   -Target clip      : clip group whose label contains ", starts at frame <Frame>,"
#   -Target trimStart : "Trim start of ..." slider whose value is <Frame>
#   -Target trimEnd   : "Trim end of ..." slider whose value is <Frame>
#   -Target window    : no element focus (global shortcuts such as Ctrl+V)
# -Keys uses System.Windows.Forms.SendKeys syntax, e.g. '+{LEFT}' (Shift+Left), '^c', '+{DEL}'.
# Keys are sent only when TinixCut owns the foreground window.
param([ValidateSet('clip','trimStart','trimEnd','window')][string]$Target, [int]$Frame = -1, [string]$Keys, [int]$Repeat = 1, [string]$Track = '')
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);'
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$el=$null
if($Target -eq 'clip'){
  $m=@($w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Group))) | Where-Object { $_.Current.Name -like "*, starts at frame $Frame,*" -and $_.Current.Name -like "*$Track*" })
  if($m.Count -ne 1){ throw "expected 1 clip starting at $Frame, found $($m.Count)" }; $el=$m[0]
} elseif($Target -ne 'window'){
  $prefix = if($Target -eq 'trimStart'){ 'Trim start of ' } else { 'Trim end of ' }
  $m=@($w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::Slider))) | Where-Object {
    $_.Current.Name.StartsWith($prefix) -and [int]$_.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).Current.Value -eq $Frame })
  if($m.Count -ne 1){ throw "expected 1 $Target slider at $Frame, found $($m.Count)" }; $el=$m[0]
}
if($el){ $el.SetFocus(); Start-Sleep -Milliseconds 150 }
$fg=[uint32]0; [void][W.U]::GetWindowThreadProcessId([W.U]::GetForegroundWindow(), [ref]$fg)
if($fg -ne [uint32]$p.Id){ throw "TinixCut is not the foreground window (foreground pid $fg); keys not sent" }
for($i=0; $i -lt $Repeat; $i++){ [System.Windows.Forms.SendKeys]::SendWait($Keys); Start-Sleep -Milliseconds 250 }
"sent '$Keys' x$Repeat to $Target $Frame"
