# Captures real pixels from the screen. Usage:
#   capture.ps1 -Out file.png -Window "LGP Battery Monitor" [-Padding 16]
#   capture.ps1 -Out file.png -Region x,y,w,h
param([string]$Out, [string]$Window, [string]$Region, [int]$Padding = 0)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W { 
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(IntPtr c, string t);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
}
"@
[W]::SetProcessDPIAware() | Out-Null
if ($Window) {
  $h = [W]::FindWindow([IntPtr]::Zero, $Window)
  if ($h -eq [IntPtr]::Zero) { throw "Window '$Window' not found" }
  $r = New-Object W+RECT
  [W]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null  # DWMWA_EXTENDED_FRAME_BOUNDS
  $x = $r.L - $Padding; $y = $r.T - $Padding; $w = $r.R - $r.L + 2*$Padding; $ht = $r.B - $r.T + 2*$Padding
} else { $x,$y,$w,$ht = $Region.Split(",") | ForEach-Object { [int]$_ } }
$bmp = New-Object System.Drawing.Bitmap $w, $ht
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"$Out ${w}x${ht} @ $x,$y"
