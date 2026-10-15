# VigilShelf README screenshot driver.
#
# Captures the panel itself via PrintWindow (flag 2 = PW_RENDERFULLCONTENT),
# which asks the window to render itself and is therefore IMMUNE TO Z-ORDER.
# Desktop clutter / other always-on-top windows can never leak into the images.
#
# The panel is first moved to a known screen position, because clicks DO go
# through the normal window stack. Panel size is fixed (PANEL_WIDTH/HEIGHT),
# so the whole geometry is deterministic: absolute = position + relative.
#
# PrintWindow paints black when the target has not painted yet, so both the
# summon and every capture are retried until they produce real pixels.

param(
  [string]$OutDir = "F:/develop/ReachFile/docs/screenshots",
  [int]$PX = 80,
  [int]$PY = 1050,
  [int]$W = 840,
  [int]$H = 405
)

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W {
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint ownerPid);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool IsHungAppWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

# Without this PowerShell is DPI-unaware: Screen.Bounds and SetCursorPos would
# speak logical pixels while Tauri and PrintWindow speak physical ones.
[void][W]::SetProcessDpiAwarenessContext([IntPtr](-4))

$ErrorActionPreference = 'Stop'
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

$HWND_TOPMOST   = [IntPtr](-1)
$SWP_NOSIZE     = 0x0001
$SWP_SHOWWINDOW = 0x0040

function Get-PanelHandle {
  $pids = @(Get-Process vigilshelf -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
  if ($pids.Count -eq 0) { return [IntPtr]::Zero }
  $script:found = [IntPtr]::Zero
  $cb = [W+EnumProc] {
    param($h, $p)
    $owner = 0
    [void][W]::GetWindowThreadProcessId($h, [ref]$owner)
    if ($pids -notcontains [int]$owner) { return $true }
    $sb = New-Object System.Text.StringBuilder 256
    [void][W]::GetClassName($h, $sb, 256)
    if ($sb.ToString() -ne 'Tauri Window') { return $true }
    $r = New-Object W+RECT
    [void][W]::GetWindowRect($h, [ref]$r)
    if (($r.Right - $r.Left) -lt 500) { return $true }
    $script:found = $h
    return $false
  }
  [void][W]::EnumWindows($cb, [IntPtr]::Zero)
  return $script:found
}

function Show-PanelState([string]$tag) {
  $r = New-Object W+RECT
  [void][W]::GetWindowRect($script:panel, [ref]$r)
  Write-Host ("  [{0}] hwnd={1} vis={2} rect=({3},{4})-({5},{6})" -f `
    $tag, $script:panel, [W]::IsWindowVisible($script:panel), $r.Left, $r.Top, $r.Right, $r.Bottom)
}

# The hot zone needs the cursor to sit on the top edge for its full delay.
function Invoke-Summon {
  for ($i = 1; $i -le 5; $i++) {
    [void][W]::SetCursorPos(1280, 0)
    Start-Sleep -Milliseconds 350
    [void][W]::SetCursorPos(1280, 1)
    Start-Sleep -Milliseconds 1300
    $script:panel = Get-PanelHandle
    if ($script:panel -eq [IntPtr]::Zero) { Write-Host "  summon ${i}: no window"; continue }
    $r = New-Object W+RECT
    [void][W]::GetWindowRect($script:panel, [ref]$r)
    $vis = [W]::IsWindowVisible($script:panel)
    Write-Host ("  summon {0}: vis={1} rect=({2},{3})-({4},{5})" -f $i, $vis, $r.Left, $r.Top, $r.Right, $r.Bottom)
    if ($vis -and ($r.Right - $r.Left) -ge 500) { return $true }
    Start-Sleep -Milliseconds 700
  }
  return $false
}

function Check-Alive([string]$tag) {
  if ($script:panel -eq [IntPtr]::Zero) { Write-Host ("  [{0}] no window" -f $tag); return }
  if ([W]::IsHungAppWindow($script:panel)) { Write-Host ("  !!! [{0}] APP IS HUNG / not responding" -f $tag) }
  else { Write-Host ("  [{0}] responding" -f $tag) }
}

function Click([int]$x, [int]$y, [bool]$right = $false) {
  [void][W]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 250
  if ($right) {
    [W]::mouse_event(0x0008, 0, 0, 0, [IntPtr]::Zero)   # RIGHTDOWN
    Start-Sleep -Milliseconds 60
    [W]::mouse_event(0x0010, 0, 0, 0, [IntPtr]::Zero)   # RIGHTUP
  } else {
    [W]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)   # LEFTDOWN
    Start-Sleep -Milliseconds 60
    [W]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)   # LEFTUP
  }
}

# Virtual key codes for letters are the UPPERCASE ASCII values. Sending a
# lowercase value produces garbage (it is not a valid VK at all).
function Tap([byte]$vk) {
  [W]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 40
  [W]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 120
}

function Chord([byte]$modifier, [byte]$vk) {
  [W]::keybd_event($modifier, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  Tap $vk
  [W]::keybd_event($modifier, 0, 2, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 250
}

# Press several keys together, e.g. the global hotkey Ctrl+Shift+E.
function Combo([byte[]]$vks) {
  foreach ($v in $vks) { [W]::keybd_event($v, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 50 }
  Start-Sleep -Milliseconds 60
  for ($i = $vks.Count - 1; $i -ge 0; $i--) { [W]::keybd_event($vks[$i], 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 50 }
  Start-Sleep -Milliseconds 250
}

$VK_CTRL  = [byte]0x11
$VK_SHIFT = [byte]0x10
$VK_E     = [byte]0x45

# The top-edge hot zone only re-anchors on a FRESH summon: while the panel is
# already up it merely auto-hides. So force a clean cycle with the global
# hotkey - one press hides, the next shows it anchored to the top edge.
function Force-TopAnchor {
  for ($press = 1; $press -le 5; $press++) {
    Combo @($VK_CTRL, $VK_SHIFT, $VK_E)
    Start-Sleep -Milliseconds 1400
    $script:panel = Get-PanelHandle
    if ($script:panel -eq [IntPtr]::Zero) { Write-Host ("  press {0}: no window" -f $press); continue }
    $r = New-Object W+RECT
    [void][W]::GetWindowRect($script:panel, [ref]$r)
    $vis = [W]::IsWindowVisible($script:panel)
    Write-Host ("  press {0}: vis={1} rect=({2},{3})-({4},{5})" -f $press, $vis, $r.Left, $r.Top, $r.Right, $r.Bottom)
    if ($vis -and $r.Top -le 8 -and ($r.Right - $r.Left) -ge 500) { return $true }
  }
  return $false
}

function Shot([string]$name) {
  for ($try = 1; $try -le 6; $try++) {
    Start-Sleep -Milliseconds 450
    $bmp = New-Object System.Drawing.Bitmap($W, $H)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc()
    $ok = [W]::PrintWindow($script:panel, $dc, 2)
    $g.ReleaseHdc($dc)
    $g.Dispose()
    $probe = $bmp.GetPixel([int]($W / 2), [int]($H / 2))
    if (($probe.R + $probe.G + $probe.B) -gt 0) {
      $path = Join-Path $OutDir $name
      $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
      $bmp.Dispose()
      Write-Host ("saved {0} (PrintWindow={1}, probe={2},{3},{4})" -f $path, $ok, $probe.R, $probe.G, $probe.B)
      return $true
    }
    $bmp.Dispose()
    Write-Host ("  {0}: black frame, retry {1}" -f $name, $try)
    [void][W]::SetForegroundWindow($script:panel)
    [void][W]::SetCursorPos(($PX + 420), ($PY + 150))
  }
  Write-Host ("FAILED {0}: six black frames" -f $name)
  return $false
}

if (-not (Force-TopAnchor)) {
  Write-Host "  hotkey cycle did not reach the top edge; trying the hot zone"
  if (-not (Invoke-Summon)) {
    Write-Host "FATAL: could not get the panel to the top edge - is vigilshelf.exe running?"
    exit 1
  }
}
Show-PanelState "summoned"

# Stay at the natural anchor position. Moving the window elsewhere was tried and
# breaks the capture: Chromium stops producing frames for an occluded WebView2
# ("occlusion detection"), so PrintWindow then paints solid black. At the top
# edge the panel is topmost and on screen, so it keeps rendering.
$rect = New-Object W+RECT
[void][W]::GetWindowRect($script:panel, [ref]$rect)
$PX = $rect.Left
$PY = $rect.Top
[void][W]::SetForegroundWindow($script:panel)
Start-Sleep -Milliseconds 300
Write-Host ("foreground = {0} (panel = {1})" -f [W]::GetForegroundWindow(), $script:panel)
# keep the cursor inside, otherwise the hot zone arms its 300ms auto-hide
[void][W]::SetCursorPos(($PX + 420), ($PY + 150))
Start-Sleep -Milliseconds 900
Show-PanelState "parked"

$ESC  = [byte]0x1B
$CTRL = [byte]0x11

# panel-relative anchors, derived from the 560x270 logical / 840x405 physical panel
$GEAR = @(760, 32)     # topbar settings button
$CARD1 = @(108, 184)   # centre of the first card

Shot "01-panel-dark.png"
Check-Alive "after 01"

# Prime the clipboard first: the menu disables "paste into this folder"
# when nothing is on the clipboard, and a greyed-out item reads like a bug.
Click ($PX + $CARD1[0]) ($PY + $CARD1[1])                 # select the first card
Start-Sleep -Milliseconds 200
Chord $CTRL ([byte]0x43)                                  # Ctrl+C
Start-Sleep -Milliseconds 2200                            # let the toast fade out

Click ($PX + $CARD1[0]) ($PY + $CARD1[1]) $true      # right-click -> context menu
Check-Alive "after right-click"
Shot "02-context-menu.png"
Tap $ESC
Start-Sleep -Milliseconds 400
Check-Alive "after esc 1"

Click ($PX + $GEAR[0]) ($PY + $GEAR[1])
Check-Alive "after gear click"
Shot "03-settings.png"
Tap $ESC
Start-Sleep -Milliseconds 400
Check-Alive "after esc 2"

Chord $CTRL ([byte]0x46)                             # Ctrl+F
Check-Alive "after ctrl+f"
# Only ONE key: the Chinese IME inserts an apostrophe between two syllable
# starts ("m" + "p" becomes "m'p"), which would filter to zero rows.
Tap ([byte]0x50)                                     # p -> 6 of 14 match
Shot "04-filter.png"
Tap $ESC
Check-Alive "after esc 3"

Write-Host "done"
