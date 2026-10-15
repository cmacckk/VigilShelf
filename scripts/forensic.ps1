Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class F {
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }

  static StringBuilder acc;
  public static string AllTauri() {
    acc = new StringBuilder();
    EnumWindows(new EnumProc(Cb), IntPtr.Zero);
    return acc.ToString();
  }
  static bool Cb(IntPtr h, IntPtr p) {
    StringBuilder cls = new StringBuilder(64); GetClassName(h, cls, 64);
    if (cls.ToString() != "Tauri Window") return true;
    RECT r; GetWindowRect(h, out r);
    uint pid; GetWindowThreadProcessId(h, out pid);
    StringBuilder txt = new StringBuilder(128); GetWindowText(h, txt, 128);
    acc.AppendLine(String.Format("  hwnd={0} pid={1} vis={2} title='{3}' rect=({4},{5})-({6},{7}) size={8}x{9}",
      h, pid, IsWindowVisible(h), txt, r.Left, r.Top, r.Right, r.Bottom, r.Right - r.Left, r.Bottom - r.Top));
    return true;
  }
  static uint target; static IntPtr found;
  public static IntPtr FindPanel(uint pid) {
    target = pid; found = IntPtr.Zero;
    EnumWindows(new EnumProc(Cb2), IntPtr.Zero);
    return found;
  }
  static bool Cb2(IntPtr h, IntPtr p) {
    uint pid; GetWindowThreadProcessId(h, out pid);
    if (pid != target) return true;
    StringBuilder cls = new StringBuilder(64); GetClassName(h, cls, 64);
    if (cls.ToString() != "Tauri Window") return true;
    RECT r; GetWindowRect(h, out r);
    if ((r.Right - r.Left) < 500) return true;
    found = h; return false;
  }
  public static RECT Rect(IntPtr h) { RECT r; GetWindowRect(h, out r); return r; }
  public static string At(int x, int y) {
    POINT p; p.X = x; p.Y = y;
    IntPtr h = WindowFromPoint(p);
    uint pid; GetWindowThreadProcessId(h, out pid);
    StringBuilder cls = new StringBuilder(64); GetClassName(h, cls, 64);
    StringBuilder txt = new StringBuilder(128); GetWindowText(h, txt, 128);
    return String.Format("hwnd={0} pid={1} class='{2}' title='{3}'", h, pid, cls, txt);
  }
  public static IntPtr Fg() { return GetForegroundWindow(); }
}
'@ -Language CSharp

[void][F]::SetProcessDpiAwarenessContext([IntPtr](-4))
$cwd = Get-Location

Write-Output "=== 所有 Tauri Window（跨进程）==="
Write-Output ([F]::AllTauri())

$procs = @{}
Get-Process | ForEach-Object { $procs[[uint32]$_.Id] = $_.ProcessName }

$mine = Get-Process vigilshelf -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $mine) { Write-Output "vigilshelf not running"; exit 1 }
$h = [F]::FindPanel([uint32]$mine.Id)
Write-Output ("MY PANEL hwnd={0} pid={1}" -f $h, $mine.Id)
$r = [F]::Rect($h)
$w = $r.Right - $r.Left; $ht = $r.Bottom - $r.Top
Write-Output ("rect=({0},{1})-({2},{3}) size={4}x{5}" -f $r.Left, $r.Top, $r.Right, $r.Bottom, $w, $ht)
Write-Output ("foreground hwnd=" + [F]::Fg())
Write-Output ("topmost at panel centre: " + [F]::At(($r.Left + [int]($w / 2)), ($r.Top + [int]($ht / 2))))

# 1) PrintWindow：让窗口自己画自己，完全无视 z 序
$bmp = New-Object System.Drawing.Bitmap -ArgumentList $w, $ht
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[void][F]::PrintWindow($h, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save((Join-Path $cwd "docs/screenshots/forensic-printwindow.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "saved forensic-printwindow.png (窗口自绘)"

# 2) 屏幕像素：受 z 序影响
$bmp2 = New-Object System.Drawing.Bitmap -ArgumentList $w, $ht
$g2 = [System.Drawing.Graphics]::FromImage($bmp2)
$g2.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size -ArgumentList $w, $ht))
$g2.Dispose()
$bmp2.Save((Join-Path $cwd "docs/screenshots/forensic-screen.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$bmp2.Dispose()
Write-Output "saved forensic-screen.png (屏幕像素)"
