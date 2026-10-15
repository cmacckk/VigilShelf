Add-Type -AssemblyName System.Windows.Forms
[void][System.Windows.Forms.SendKeys]::SendWait("^+e")
Write-Output "sent Ctrl+Shift+E"
