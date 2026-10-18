# Original geometric artwork for Express MSI Builder. No external image assets.
Add-Type -AssemblyName System.Drawing
$assetDir = Join-Path $PSScriptRoot '../assets/installer'
[IO.Directory]::CreateDirectory($assetDir) | Out-Null

function Draw-Package($graphics, [single]$x, [single]$y, [single]$scale) {
    $edge = [Drawing.Pen]::new([Drawing.Color]::FromArgb(210, 236, 247), 2 * $scale)
    $top = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(90, 223, 202))
    $left = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(41, 115, 155))
    $right = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(25, 77, 119))
    try {
        $a = [Drawing.PointF]::new($x, $y)
        $b = [Drawing.PointF]::new($x + 62*$scale, $y + 34*$scale)
        $c = [Drawing.PointF]::new($x, $y + 68*$scale)
        $d = [Drawing.PointF]::new($x - 62*$scale, $y + 34*$scale)
        $e = [Drawing.PointF]::new($x - 62*$scale, $y + 104*$scale)
        $f = [Drawing.PointF]::new($x, $y + 138*$scale)
        $h = [Drawing.PointF]::new($x + 62*$scale, $y + 104*$scale)
        $graphics.FillPolygon($top, [Drawing.PointF[]]@($a,$b,$c,$d))
        $graphics.FillPolygon($left, [Drawing.PointF[]]@($d,$c,$f,$e))
        $graphics.FillPolygon($right, [Drawing.PointF[]]@($c,$b,$h,$f))
        $graphics.DrawPolygon($edge, [Drawing.PointF[]]@($a,$b,$h,$f,$e,$d))
        $graphics.DrawLines($edge, [Drawing.PointF[]]@($d,$c,$b))
        $graphics.DrawLine($edge,$c,$f)
    } finally { $edge.Dispose(); $top.Dispose(); $left.Dispose(); $right.Dispose() }
}

foreach ($kind in @('welcome','banner')) {
    $width = if ($kind -eq 'welcome') {260} else {780}
    $height = if ($kind -eq 'welcome') {470} else {104}
    $image = [Drawing.Bitmap]::new($width,$height,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [Drawing.Graphics]::FromImage($image)
    $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    try {
        if ($kind -eq 'welcome') {
            $gradient = [Drawing.Drawing2D.LinearGradientBrush]::new(
                [Drawing.Rectangle]::new(0,0,$width,$height),
                [Drawing.Color]::FromArgb(13,31,57), [Drawing.Color]::FromArgb(24,83,112), 55)
            $g.FillRectangle($gradient,0,0,$width,$height)
            $gradient.Dispose()
            $grid = [Drawing.Pen]::new([Drawing.Color]::FromArgb(22,180,226,235),1)
            for ($i = -400; $i -lt 600; $i += 38) { $g.DrawLine($grid,$i,0,($i+250),470) }
            $grid.Dispose()
            $accent = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(90,223,202))
            $g.FillRectangle($accent,28,34,42,5)
            $g.FillRectangle($accent,28,419,7,7)
            $accent.Dispose()
            Draw-Package $g 130 148 1.25
        } else {
            $g.Clear([Drawing.Color]::White)
            $pale = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(232,246,248))
            $g.FillPolygon($pale,[Drawing.Point[]]@([Drawing.Point]::new(620,0),[Drawing.Point]::new(780,0),[Drawing.Point]::new(780,104),[Drawing.Point]::new(575,104)))
            $pale.Dispose()
            Draw-Package $g 702 16 0.62
        }
        $image.Save((Join-Path $assetDir "$kind.bmp"),[Drawing.Imaging.ImageFormat]::Bmp)
        $image.Save((Join-Path $assetDir "$kind-preview.png"),[Drawing.Imaging.ImageFormat]::Png)
    } finally { $g.Dispose(); $image.Dispose() }
}
