param(
    [string]$Publisher = $(if ($env:ORBIT_MSIX_PUBLISHER) { $env:ORBIT_MSIX_PUBLISHER } else { 'CN=Verruckt' }),
    [string]$IdentityName = $(if ($env:ORBIT_MSIX_IDENTITY_NAME) { $env:ORBIT_MSIX_IDENTITY_NAME } else { 'app.verruckt.desktop' }),
    [string]$PublisherDisplayName = $(if ($env:ORBIT_MSIX_PUBLISHER_DISPLAY_NAME) { $env:ORBIT_MSIX_PUBLISHER_DISPLAY_NAME } else { 'Verruckt' }),
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'src-tauri\target\release\verruckt.exe'
$sourceIcon = Join-Path $root 'src-tauri\icons\128x128@2x.png'
$stage = Join-Path $root '.orbit-server\msix\Orbit'
$assets = Join-Path $stage 'Assets'
$outDir = Join-Path $root 'src-tauri\target\release\bundle\msix'

if (-not $Version) {
    $tauriConfig = Get-Content -LiteralPath (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
    $baseVersion = $tauriConfig.bundle.windows.wix.version
    if (-not $baseVersion) {
        $packageJson = Get-Content -LiteralPath (Join-Path $root 'package.json') -Raw | ConvertFrom-Json
        $baseVersion = ($packageJson.version -split '-')[0]
    }
    if ($baseVersion -match '^\d+\.\d+\.\d+\.\d+$') {
        $Version = $baseVersion
    } else {
        $Version = "$baseVersion.0"
    }
}
$out = Join-Path $outDir "Orbit_${Version}_x64.msix"

if (-not (Test-Path -LiteralPath $exe)) {
    throw "Missing $exe. Run 'npm run tauri build --bundles nsis,msi' first."
}
if (-not (Test-Path -LiteralPath $sourceIcon)) {
    throw "Missing icon $sourceIcon."
}

Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $assets | Out-Null
Copy-Item -LiteralPath $exe -Destination $stage
Copy-Item -LiteralPath $sourceIcon -Destination (Join-Path $assets 'source.png')

Add-Type -AssemblyName System.Drawing

function New-OrbitAsset {
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

$icon = Join-Path $assets 'source.png'
New-OrbitAsset -Source $icon -Destination (Join-Path $assets 'StoreLogo.png') -Width 50 -Height 50
New-OrbitAsset -Source $icon -Destination (Join-Path $assets 'Square44x44Logo.png') -Width 44 -Height 44
New-OrbitAsset -Source $icon -Destination (Join-Path $assets 'Square150x150Logo.png') -Width 150 -Height 150
New-OrbitAsset -Source $icon -Destination (Join-Path $assets 'Wide310x150Logo.png') -Width 310 -Height 150
New-OrbitAsset -Source $icon -Destination (Join-Path $assets 'SplashScreen.png') -Width 620 -Height 300
Remove-Item -LiteralPath $icon -Force

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:mp="http://schemas.microsoft.com/appx/2014/phone/manifest"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap mp rescap">
  <Identity Name="$IdentityName" Publisher="$Publisher" Version="$Version" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Orbit</DisplayName>
    <PublisherDisplayName>$PublisherDisplayName</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
    <Description>把好奇心变成一条不断生长的学习路径。</Description>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources>
    <Resource Language="zh-CN" />
  </Resources>
  <Applications>
    <Application Id="Orbit" Executable="verruckt.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="Orbit"
        Description="把好奇心变成一条不断生长的学习路径。"
        BackgroundColor="#0B0F19"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png" />
        <uap:SplashScreen Image="Assets\SplashScreen.png" BackgroundColor="#0B0F19" />
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
Set-Content -LiteralPath (Join-Path $stage 'AppxManifest.xml') -Value $manifest -Encoding utf8

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $makeappx) {
    throw 'makeappx.exe not found. Install the Windows 10/11 SDK.'
}

New-Item -ItemType Directory -Force -Path $outDir | Out-Null
& $makeappx.FullName pack /o /d $stage /p $out
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed with exit code $LASTEXITCODE" }

$unpacked = Join-Path $root '.orbit-server\msix\unpacked'
Remove-Item -LiteralPath $unpacked -Recurse -Force -ErrorAction SilentlyContinue
& $makeappx.FullName unpack /o /p $out /d $unpacked
if ($LASTEXITCODE -ne 0) { throw "makeappx unpack validation failed with exit code $LASTEXITCODE" }
Remove-Item -LiteralPath $unpacked -Recurse -Force -ErrorAction SilentlyContinue

if ($env:ORBIT_MSIX_PFX) {
    if (-not (Test-Path -LiteralPath $env:ORBIT_MSIX_PFX)) {
        throw "ORBIT_MSIX_PFX not found: $env:ORBIT_MSIX_PFX"
    }
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $signtool) { throw 'signtool.exe not found. Install the Windows 10/11 SDK.' }
    $signArgs = @('sign', '/fd', 'SHA256', '/f', $env:ORBIT_MSIX_PFX)
    if ($env:ORBIT_MSIX_PFX_PASSWORD) {
        $signArgs += @('/p', $env:ORBIT_MSIX_PFX_PASSWORD)
    }
    $signArgs += $out
    & $signtool.FullName @signArgs
    if ($LASTEXITCODE -ne 0) { throw "signtool failed with exit code $LASTEXITCODE" }
    Write-Host 'MSIX signed.'
}
else {
    Write-Host 'MSIX created unsigned. Set ORBIT_MSIX_PFX (and ORBIT_MSIX_PFX_PASSWORD) to sign.' -ForegroundColor Yellow
}

Write-Host "MSIX: $out"
