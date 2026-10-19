# Derives the extra sizes the packaging needs from the brand art in
# assets/brand (windows PowerShell 5.1, System.Drawing only):
#   assets/brand/app-512.png, gdr-512.png (macOS iconset)
#   packaging/windows/msix/Assets/*.png (Store logos, .gdr file logo)
# Run from anywhere: powershell -File scripts/brand/make-assets.ps1
Add-Type -AssemblyName System.Drawing
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$brand = Join-Path $root 'assets\brand'
$msix = Join-Path $root 'packaging\windows\msix\Assets'

function Save-Scaled($src, $dst, $w, $h, $box) {
    $img = [System.Drawing.Image]::FromFile($src)
    $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $side = [Math]::Min($box, [Math]::Min($w, $h))
    $g.DrawImage($img, [int](($w - $side) / 2), [int](($h - $side) / 2), $side, $side)
    $g.Dispose(); $img.Dispose()
    $bmp.Save($dst, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

$app = Join-Path $brand 'app-1024.png'
$gdr = Join-Path $brand 'gdr-1024.png'
Save-Scaled $app (Join-Path $brand 'app-512.png') 512 512 512
Save-Scaled $gdr (Join-Path $brand 'gdr-512.png') 512 512 512
Save-Scaled $app (Join-Path $msix 'Square44x44Logo.png') 44 44 44
Save-Scaled $app (Join-Path $msix 'StoreLogo.png') 50 50 50
Save-Scaled $app (Join-Path $msix 'Square150x150Logo.png') 150 150 150
Save-Scaled $app (Join-Path $msix 'Square310x310Logo.png') 310 310 310
Save-Scaled $app (Join-Path $msix 'Wide310x150Logo.png') 310 150 150
Save-Scaled $gdr (Join-Path $msix 'GdrFileLogo.png') 256 256 256

# Splash art: scripts/brand/make-assets.ps1 -Splash <source.jpg> writes
# assets/brand/splash.jpg (1376 px wide, JPEG quality 80).
if ($args.Count -ge 2 -and $args[0] -eq '-Splash') {
    $src = [System.Drawing.Image]::FromFile($args[1])
    $w = 1376; $h = [int][Math]::Round($src.Height * $w / $src.Width)
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($src, 0, 0, $w, $h); $g.Dispose(); $src.Dispose()
    $codec = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
    $ep = New-Object System.Drawing.Imaging.EncoderParameters 1
    $ep.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter ([System.Drawing.Imaging.Encoder]::Quality), 80L
    $bmp.Save((Join-Path $brand 'splash.jpg'), $codec, $ep); $bmp.Dispose()
}
