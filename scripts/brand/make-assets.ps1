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
