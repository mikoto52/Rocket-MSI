# Render the installer cube from the shared vector geometry, with no background.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
. (Join-Path $PSScriptRoot 'draw-package.ps1')
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$assets = Join-Path $root 'assets/installer'
$frames = @()
foreach ($size in @(16,24,32,48,64,128,256)) {
    $large = [Drawing.Bitmap]::new($size*4,$size*4,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $image = [Drawing.Bitmap]::new($size,$size,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($large)
    $scaled = [Drawing.Graphics]::FromImage($image)
    $stream = [IO.MemoryStream]::new()
    try {
        $g.Clear([Drawing.Color]::Transparent)
        $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $margin = $size*0.06*4
        Draw-Package $g ($size*2) $margin (($size*4-2*$margin)/138)
        $scaled.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $scaled.DrawImage($large,0,0,$size,$size)
        $image.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
        $frames += [pscustomobject]@{Size=$size;Bytes=$stream.ToArray()}
        if ($size -eq 256) { $image.Save((Join-Path $assets 'app-icon.png'),[Drawing.Imaging.ImageFormat]::Png) }
    } finally { $stream.Dispose();$scaled.Dispose();$g.Dispose();$large.Dispose();$image.Dispose() }
}
$file = [IO.File]::Create((Join-Path $assets 'app-icon.ico'))
$writer = [IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]$frames.Count)
    $offset = 6 + 16*$frames.Count
    foreach ($frame in $frames) {
        $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
        $writer.Write([byte]$dimension);$writer.Write([byte]$dimension)
        $writer.Write([byte]0);$writer.Write([byte]0)
        $writer.Write([uint16]1);$writer.Write([uint16]32)
        $writer.Write([uint32]$frame.Bytes.Length);$writer.Write([uint32]$offset)
        $offset += $frame.Bytes.Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
} finally { $writer.Dispose();$file.Dispose() }
Copy-Item -LiteralPath (Join-Path $assets 'app-icon.png') -Destination (Join-Path $root 'builder/icons/icon.png')
Copy-Item -LiteralPath (Join-Path $assets 'app-icon.ico') -Destination (Join-Path $root 'builder/icons/icon.ico')
Copy-Item -LiteralPath (Join-Path $assets 'app-icon.png') -Destination (Join-Path $root 'builder/ui/icon.png')
Write-Host 'Generated InstallEmbly cube PNG and multi-resolution ICO (16..256).'
