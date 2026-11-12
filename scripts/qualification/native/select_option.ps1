# Select an option in a native-exposed <select> (UIA ComboBox) that currently contains -Option.
param([string]$Option)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A=[System.Windows.Automation.AutomationElement]; $T=[System.Windows.Automation.TreeScope]; $C=[System.Windows.Automation.PropertyCondition]
$p=Get-Process tinixcut | Select-Object -First 1
$w=$A::RootElement.FindFirst($T::Children,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ProcessIdProperty,[int]$p.Id)),(New-Object $C($A::ClassNameProperty,'Tauri Window')))))
$items=@($w.FindAll($T::Descendants,(New-Object System.Windows.Automation.AndCondition((New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::ListItem)),(New-Object $C($A::NameProperty,$Option))))))
if($items.Count -ne 1){
  # Options are only exposed after the combo box is expanded.
  $combos=@($w.FindAll($T::Descendants,(New-Object $C($A::ControlTypeProperty,[System.Windows.Automation.ControlType]::ComboBox))))
  foreach($cb in $combos){
    try { $cb.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand(); Start-Sleep -Milliseconds 200 } catch { continue }
    $items=@($cb.FindAll($T::Descendants,(New-Object $C($A::NameProperty,$Option))))
    if($items.Count -eq 1){ break }
    try { $cb.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Collapse() } catch {}
  }
}
if($items.Count -ne 1){ throw "option '$Option' not found uniquely" }
$items[0].GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select(); Start-Sleep -Milliseconds 300
"selected $Option"
