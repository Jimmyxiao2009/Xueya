param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$sourceIcon = Join-Path $root 'src-tauri\icons\128x128@2x.png'
$outDir = Join-Path $root 'store'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

if (-not (Test-Path -LiteralPath $sourceIcon)) {
    throw "Missing source icon: $sourceIcon"
}

Add-Type -AssemblyName System.Drawing

function New-Asset {
    param(
        [Parameter(Mandatory)] [string]$Source,
        [Parameter(Mandatory)] [string]$Destination,
        [Parameter(Mandatory)] [int]$Width,
        [Parameter(Mandatory)] [int]$Height
    )

    $image = [System.Drawing.Image]::FromFile($Source)
    try {
        $bitmap = New-Object System.Drawing.Bitmap($Width, $Height)
        try {
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            try {
                $graphics.Clear([System.Drawing.Color]::FromArgb(11, 15, 25))
                $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
                $scale = [Math]::Min($Width / $image.Width, $Height / $image.Height) * 0.78
                $drawWidth = [Math]::Max(1, [int]($image.Width * $scale))
                $drawHeight = [Math]::Max(1, [int]($image.Height * $scale))
                $x = [int](($Width - $drawWidth) / 2)
                $y = [int](($Height - $drawHeight) / 2)
                $graphics.DrawImage($image, $x, $y, $drawWidth, $drawHeight)
            }
            finally {
                $graphics.Dispose()
            }
            $bitmap.Save($Destination, [System.Drawing.Imaging.ImageFormat]::Png)
        }
        finally {
            $bitmap.Dispose()
        }
    }
    finally {
        $image.Dispose()
    }
}

New-Asset -Source $sourceIcon -Destination (Join-Path $outDir 'Orbit_300x300.png') -Width 300 -Height 300
New-Asset -Source $sourceIcon -Destination (Join-Path $outDir 'Orbit_150x150.png') -Width 150 -Height 150
New-Asset -Source $sourceIcon -Destination (Join-Path $outDir 'Orbit_71x71.png') -Width 71 -Height 71

Write-Host "Store assets written to $outDir"
