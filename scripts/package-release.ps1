param(
    [string]$Version,
    [string]$OutputDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$packageJson = Get-Content -LiteralPath (Join-Path $root 'package.json') -Raw | ConvertFrom-Json
if (-not $Version) {
    $Version = $packageJson.version
}
if (-not $OutputDir) {
    $OutputDir = Join-Path $root '.orbit-server\release'
}
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$imagesTar = Join-Path $root '.orbit-server\orbit-demo-images.tar'
if (-not (Test-Path -LiteralPath $imagesTar)) {
    $fallback = Join-Path $root 'deploy\orbit-demo-images.tar'
    if (Test-Path -LiteralPath $fallback) {
        $imagesTar = $fallback
    } else {
        throw "Missing offline image bundle. Expected $imagesTar"
    }
}

function Find-BundleArtifact {
    param(
        [Parameter(Mandatory)] [string]$Directory,
        [Parameter(Mandatory)] [string]$Filter
    )
    if (-not (Test-Path -LiteralPath $Directory)) {
        throw "Missing bundle directory: $Directory"
    }
    $artifact = Get-ChildItem -LiteralPath $Directory -Filter $Filter -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $artifact) {
        throw "Missing installer matching $Filter in $Directory"
    }
    return $artifact.FullName
}

$nsis = Find-BundleArtifact -Directory (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter '*x64-setup.exe'
$msi = Find-BundleArtifact -Directory (Join-Path $root 'src-tauri\target\release\bundle\msi') -Filter '*x64_en-US.msi'
$msix = Find-BundleArtifact -Directory (Join-Path $root 'src-tauri\target\release\bundle\msix') -Filter '*_x64.msix'

$serverStage = Join-Path $OutputDir "orbit-deploy-$Version"
$windowsStage = Join-Path $OutputDir "orbit-windows-$Version"
foreach ($stage in @($serverStage, $windowsStage)) {
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
}

# Server/offline deployment bundle.
Copy-Item -LiteralPath (Join-Path $root 'deploy\compose.yml') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\.env.example') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\Dockerfile') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\deploy.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\deploy.ps1') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\preflight.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\install-docker-ubuntu.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\init-env.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\bootstrap.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\backup.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\restore.sh') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\Caddyfile.example') -Destination $serverStage
Copy-Item -LiteralPath (Join-Path $root 'deploy\README.md') -Destination $serverStage
Copy-Item -LiteralPath $imagesTar -Destination (Join-Path $serverStage 'orbit-demo-images.tar')
Copy-Item -LiteralPath (Join-Path $root 'scripts\deploy-remote.ps1') -Destination (Join-Path $serverStage 'deploy-remote.ps1')
Copy-Item -LiteralPath (Join-Path $root 'scripts\smoke-production.ps1') -Destination (Join-Path $serverStage 'smoke-production.ps1')
Copy-Item -LiteralPath (Join-Path $root 'docs\HANDOFF.md') -Destination (Join-Path $serverStage 'HANDOFF.md')
Copy-Item -LiteralPath (Join-Path $root 'docs\LOCAL_DEMO.md') -Destination (Join-Path $serverStage 'LOCAL_DEMO.md')

# Windows installer bundle.
Copy-Item -LiteralPath $nsis -Destination (Join-Path $windowsStage ([System.IO.Path]::GetFileName($nsis)))
Copy-Item -LiteralPath $msi -Destination (Join-Path $windowsStage ([System.IO.Path]::GetFileName($msi)))
Copy-Item -LiteralPath $msix -Destination (Join-Path $windowsStage ([System.IO.Path]::GetFileName($msix)))
Copy-Item -LiteralPath (Join-Path $root 'docs\STORE.md') -Destination (Join-Path $windowsStage 'STORE.md')
Copy-Item -LiteralPath (Join-Path $root 'store\README.md') -Destination (Join-Path $windowsStage 'STORE-ASSETS.md')
Copy-Item -LiteralPath (Join-Path $root 'docs\PRIVACY.md') -Destination (Join-Path $windowsStage 'PRIVACY.md')
Copy-Item -LiteralPath (Join-Path $root 'docs\TERMS.md') -Destination (Join-Path $windowsStage 'TERMS.md')
Copy-Item -LiteralPath (Join-Path $root 'docs\SUPPORT.md') -Destination (Join-Path $windowsStage 'SUPPORT.md')
Copy-Item -LiteralPath (Join-Path $root 'scripts\sign-windows.ps1') -Destination (Join-Path $windowsStage 'sign-windows.ps1')
Copy-Item -LiteralPath (Join-Path $root 'scripts\check-store-release.ps1') -Destination (Join-Path $windowsStage 'check-store-release.ps1')
Copy-Item -LiteralPath (Join-Path $root 'store\Orbit_300x300.png') -Destination (Join-Path $windowsStage 'Orbit_300x300.png')
Copy-Item -LiteralPath (Join-Path $root 'store\Orbit_150x150.png') -Destination (Join-Path $windowsStage 'Orbit_150x150.png')
Copy-Item -LiteralPath (Join-Path $root 'store\Orbit_71x71.png') -Destination (Join-Path $windowsStage 'Orbit_71x71.png')
Copy-Item -LiteralPath (Join-Path $root 'store\screenshots') -Destination (Join-Path $windowsStage 'screenshots') -Recurse
Copy-Item -LiteralPath (Join-Path $root 'store\site') -Destination (Join-Path $windowsStage 'store-site') -Recurse

$serverArchive = Join-Path $OutputDir "orbit-deploy-$Version.tar.gz"
$windowsArchive = Join-Path $OutputDir "orbit-windows-$Version.zip"
Remove-Item -LiteralPath $serverArchive, $windowsArchive -Force -ErrorAction SilentlyContinue

Write-Host 'Creating server offline bundle...'
& tar.exe -czf $serverArchive -C $OutputDir "orbit-deploy-$Version"
if ($LASTEXITCODE -ne 0) { throw "tar failed with exit code $LASTEXITCODE" }

Write-Host 'Creating Windows installer bundle...'
Compress-Archive -Path (Join-Path $windowsStage '*') -DestinationPath $windowsArchive -CompressionLevel Optimal -Force

$sums = @(
    (Get-FileHash -LiteralPath $serverArchive -Algorithm SHA256),
    (Get-FileHash -LiteralPath $windowsArchive -Algorithm SHA256)
) | ForEach-Object { "$($_.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($_.Path))" }
$sumsPath = Join-Path $OutputDir 'SHA256SUMS.txt'
$sums | Set-Content -LiteralPath $sumsPath -Encoding ascii

Write-Host ''
Write-Host "Server bundle:  $serverArchive"
Write-Host "Windows bundle: $windowsArchive"
Write-Host "Checksums:      $sumsPath"
Get-Item -LiteralPath $serverArchive, $windowsArchive | Select-Object Name, Length | Format-Table -AutoSize
