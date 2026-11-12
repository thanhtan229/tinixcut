param([string]$Path, [string]$EditId = '1148')
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public static class W { [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h,int m,IntPtr w,string l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h,int m,IntPtr w,StringBuilder l);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,int m,IntPtr w,IntPtr l); }
"@
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$proc=Get-Process tinixcut | Select-Object -First 1
$win=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$proc.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$dlg=$win.FindFirst($T::Children,(New-Object $C($A::ClassNameProperty,'#32770')))
$all=$dlg.FindAll($T::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
$edit=$all | Where-Object { $_.Current.AutomationId -eq $EditId -and $_.Current.ClassName -eq 'Edit' } | Select-Object -First 1
$ok=$all | Where-Object { $_.Current.AutomationId -eq '1' -and $_.Current.ClassName -eq 'Button' } | Select-Object -First 1
$h=[IntPtr]$edit.Current.NativeWindowHandle
[void][W]::SendMessage($h,0x000C,[IntPtr]::Zero,$Path)
$sb=New-Object System.Text.StringBuilder 1024; [void][W]::SendMessage($h,0x000D,[IntPtr]1024,$sb)
"Filename readback: $($sb.ToString())"
if($sb.ToString() -ne $Path){ throw "readback mismatch" }
[void][W]::SendMessage([IntPtr]$ok.Current.NativeWindowHandle,0x00F5,[IntPtr]::Zero,[IntPtr]::Zero); "Clicked $($ok.Current.Name)"
