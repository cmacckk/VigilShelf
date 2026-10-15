# 冒烟截图：把光标怼到主屏最上沿触发顶边热区，等面板滑出后截屏。
#
# 为什么要显式设置 DPI 感知：PowerShell 默认是 DPI 不感知的，
# 那样 `Screen.Bounds` 返回的是缩放后的逻辑分辨率，而 Tauri 用物理像素定位窗口、
# `SetCursorPos` 也收物理像素 —— 在 125%/150% 缩放的屏幕上三者会对不上。
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Dpi {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
"@

# DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
[void][Dpi]::SetProcessDpiAwarenessContext([IntPtr](-4))

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$origin = [System.Drawing.Point]::new(0, 0)

# 光标本来的位置，跑完要放回去，别把用户的鼠标丢在屏幕角上
$before = [System.Windows.Forms.Cursor]::Position

# 顶边热区在屏幕最上沿，x 取中间
[void][Dpi]::SetCursorPos([int]($screen.Width / 2), 0)
Start-Sleep -Milliseconds 1200

$bitmap = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($origin, $origin, $bitmap.Size)
$out = Join-Path $PSScriptRoot "smoke-shot.png"
$bitmap.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()

[void][Dpi]::SetCursorPos($before.X, $before.Y)

Write-Output $out
Write-Output "$($screen.Width)x$($screen.Height)"
