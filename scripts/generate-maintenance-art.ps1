# Original maintenance symbols using the InstallEmbly blue and mint palette.
Add-Type -AssemblyName System.Drawing
$assetDir=Join-Path $PSScriptRoot '../assets/installer'
foreach($kind in @('repair','remove')) {
    $image=[Drawing.Bitmap]::new(64,64)
    $g=[Drawing.Graphics]::FromImage($image)
    $g.SmoothingMode=[Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $blue=[Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(41,115,155))
    $mint=[Drawing.Pen]::new([Drawing.Color]::FromArgb(90,223,202),4)
    $line=[Drawing.Pen]::new([Drawing.Color]::FromArgb(41,115,155),5)
    try {
        $g.Clear([Drawing.Color]::FromArgb(240,240,240))
        if($kind -eq 'repair') {
            $g.FillPolygon($blue,[Drawing.Point[]]@(
                [Drawing.Point]::new(12,47),[Drawing.Point]::new(35,24),
                [Drawing.Point]::new(34,13),[Drawing.Point]::new(43,6),
                [Drawing.Point]::new(43,18),[Drawing.Point]::new(50,22),
                [Drawing.Point]::new(58,15),[Drawing.Point]::new(57,28),
                [Drawing.Point]::new(47,34),[Drawing.Point]::new(39,33),
                [Drawing.Point]::new(21,56)))
            $g.FillEllipse($blue,9,44,16,16)
            $g.DrawEllipse($mint,14,49,6,6)
        } else {
            $g.FillRectangle($blue,17,21,31,36)
            $g.DrawLine($line,12,15,53,15)
            $g.DrawLine($line,25,8,40,8)
            $g.DrawLine($mint,27,29,27,49)
            $g.DrawLine($mint,38,29,38,49)
        }
        $image.Save((Join-Path $assetDir "$kind.bmp"),[Drawing.Imaging.ImageFormat]::Bmp)
        $image.Save((Join-Path $assetDir "$kind-preview.png"),[Drawing.Imaging.ImageFormat]::Png)
    } finally { $line.Dispose(); $mint.Dispose(); $blue.Dispose(); $g.Dispose(); $image.Dispose() }
}
