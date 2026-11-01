param(
    [string]$InstallerPath,
    [int]$LaunchWaitSeconds = 8
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ProgressPreference = 'SilentlyContinue'

$root = Split-Path -Parent $PSScriptRoot
if (-not $InstallerPath) {
    $InstallerPath = Join-Path $root 'src-tauri\target\release\bundle\nsis\Orbit_0.1.0-rc.1_x64-setup.exe'
}
if (-not (Test-Path -LiteralPath $InstallerPath)) {
    throw "Missing NSIS installer: $InstallerPath"
}

function Get-OrbitInstallDir {
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Orbit'
    if (Test-Path -LiteralPath $key) {
        $props = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
        if ($props.InstallLocation) {
            return ($props.InstallLocation -replace '"', '').Trim()
        }
    }
    return (Join-Path $env:LOCALAPPDATA 'Orbit')
}

function Remove-OrbitInstall {
    $dir = Get-OrbitInstallDir
    $uninstaller = Join-Path $dir 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            Write-Warning "uninstall.exe exited with $($process.ExitCode)"
        }
    }
}

Write-Host "Installing NSIS silently: $InstallerPath"
$install = Start-Process -FilePath $InstallerPath -ArgumentList '/S' -Wait -PassThru
if ($install.ExitCode -ne 0) {
    throw "NSIS installer exited with $($install.ExitCode)"
}

$installDir = Get-OrbitInstallDir
$exe = Join-Path $installDir 'verruckt.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw "Installed executable not found: $exe"
}
Write-Host "Installed to: $installDir"

try {
    $process = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds $LaunchWaitSeconds
    $running = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
    if (-not $running) {
        throw 'Installed verruckt.exe did not stay running.'
    }
    Write-Host "Installed app running: pid=$($process.Id)"
    Write-Host 'NSIS install/launch test passed.'
}
finally {
    Get-Process -Name 'verruckt' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Write-Host 'Uninstalling NSIS package...'
    Remove-OrbitInstall
    Start-Sleep -Seconds 2
    $remaining = Get-OrbitInstallDir
    if (Test-Path -LiteralPath $remaining) {
        Write-Warning "Install directory still exists: $remaining"
    } else {
        Write-Host 'NSIS installation removed.'
    }
}
