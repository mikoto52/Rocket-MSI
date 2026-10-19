# Shared original InstallEmbly cube geometry.
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

