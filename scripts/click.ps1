# Trigger the top-edge hot zone, then click a point inside the panel and screenshot.
# ASCII-only on purpose: PowerShell 5.1 reads .ps1 as ANSI when there is no BOM,
# so non-ASCII comments can corrupt the parse. Keep this file ASCII.
param(
    [Parameter(Mandatory = $true)][int]$X,
    [Parameter(Mandatory = $true)][int]$Y,
    [string]$Out = "scripts/click-shot.png"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Inp {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
"@

[void][Inp]::SetProcessDpiAwarenessContext([IntPtr](-4))
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$MOUSEEVENTF_LEFTDOWN = 0x0002
$MOUSEEVENTF_LEFTUP = 0x0004

$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$origin = [System.Windows.Forms.Cursor]::Position
$centerX = [int]($bounds.Width / 2)

# Step 1: park the cursor away from the panel first, so a stale "already visible"
# state cannot mask a hot-zone failure.
[void][Inp]::SetCursorPos($centerX, [int]($bounds.Height / 2))
Start-Sleep -Milliseconds 900

# Step 2: hit the top edge and wait out hotzone_delay_ms.
[void][Inp]::SetCursorPos($centerX, 0)
Start-Sleep -Milliseconds 1400

# Step 3: click the target point inside the panel.
[void][Inp]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 300
[Inp]::mouse_event($MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
[Inp]::mouse_event($MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 1000

$bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen(0, 0, 0, 0, $bitmap.Size)
$full = Join-Path (Get-Location) $Out
$bitmap.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()

[void][Inp]::SetCursorPos($origin.X, $origin.Y)

Write-Output $full
