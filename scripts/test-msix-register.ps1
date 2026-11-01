param(
    [switch]$NoBuild,
    [int]$LaunchWaitSeconds = 8
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ProgressPreference = 'SilentlyContinue'

$root = Split-Path -Parent $PSScriptRoot
$buildScript = Join-Path $PSScriptRoot 'build-msix.ps1'
$staging = Join-Path $root '.orbit-server\msix\Orbit'
$manifest = Join-Path $staging 'AppxManifest.xml'

if (-not $NoBuild) {
    & $buildScript
    if ($LASTEXITCODE -ne 0) { throw "build-msix.ps1 failed with exit code $LASTEXITCODE" }
}
if (-not (Test-Path -LiteralPath $manifest)) {
    throw "Missing $manifest. Run scripts/build-msix.ps1 first."
}

$packageName = 'app.verruckt.desktop'
$existing = Get-AppxPackage -Name $packageName -ErrorAction SilentlyContinue
if ($existing) {
    Remove-AppxPackage -Package $existing.PackageFullName -ErrorAction SilentlyContinue
}

try {
    Write-Host 'Registering loose MSIX layout...'
    Add-AppxPackage -Register $manifest -ErrorAction Stop

    $pkg = Get-AppxPackage -Name $packageName -ErrorAction Stop
    Write-Host "Registered: $($pkg.PackageFullName)"

    $launch = "shell:AppsFolder\$($pkg.PackageFamilyName)!Orbit"
    Write-Host "Launching: $launch"
    Start-Process $launch

    Start-Sleep -Seconds $LaunchWaitSeconds
    $process = Get-Process -Name 'verruckt' -ErrorAction SilentlyContinue
    if (-not $process) {
        throw 'verruckt.exe did not start after package activation.'
    }

    Write-Host "Desktop process running: pid=$($process.Id)"
    Write-Host 'MSIX loose-registration launch test passed.'
}
finally {
    Get-Process -Name 'verruckt' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    $installed = Get-AppxPackage -Name $packageName -ErrorAction SilentlyContinue
    if ($installed) {
        Remove-AppxPackage -Package $installed.PackageFullName -ErrorAction SilentlyContinue
        Write-Host 'Removed test MSIX registration.'
    }
}
