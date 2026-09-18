# scripts/android-install.ps1
# Push APK to connected physical Android device and launch

param(
    [string]$ApkPath = "",
    [switch]$FollowLogs = $false
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  VERRUCKT ANDROID APK INSTALLER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Find APK if not provided
if (-not $ApkPath) {
    $apkCandidates = Get-ChildItem -Path "src-tauri/gen/android/app/build/outputs/apk" -Filter "*.apk" -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch "unaligned" }
    $releaseApk = $apkCandidates | Where-Object { $_.FullName -match "release" } | Select-Object -First 1
    if ($releaseApk) {
        $ApkPath = $releaseApk.FullName
    } else {
        $debugApk = $apkCandidates | Where-Object { $_.FullName -match "debug" } | Select-Object -First 1
        if ($debugApk) {
            $ApkPath = $debugApk.FullName
        }
    }
}

if (-not $ApkPath -or -not (Test-Path $ApkPath)) {
    Write-Error "No APK found at '$ApkPath'! Build an APK first or specify -ApkPath <path>."
    exit 1
}

Write-Host "Target APK: $ApkPath" -ForegroundColor Green

# Check ADB
$adbCmd = Get-Command adb -ErrorAction SilentlyContinue
$adbBin = if ($adbCmd) { $adbCmd.Source } else { $null }
if (-not $adbBin) {
    $candidate = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
    if (Test-Path $candidate) { $adbBin = $candidate }
}
if (-not $adbBin) {
    Write-Error "adb.exe not found!"
    exit 1
}

# Verify device
$devices = & $adbBin devices -l
$connected = $devices | Where-Object { $_ -match "^\s*([a-zA-Z0-9\._\-:]+)\s+device\b" }
if (-not $connected) {
    Write-Error "No authorized Android device connected! Connect phone, enable USB debugging, and accept authorization."
    exit 1
}
$devSerial = ($connected[0] -split '\s+')[0]
Write-Host "Target Device: $devSerial" -ForegroundColor Green

# Install APK
Write-Host "`nInstalling APK via adb install -r..." -ForegroundColor Yellow
$installRes = & $adbBin -s $devSerial install -r $ApkPath 2>&1
Write-Host ($installRes -join "`n")
if ($installRes -notmatch "Success") {
    Write-Error "APK installation failed!"
    exit 1
}

# Launch App
$packageName = "app.verruckt.desktop"
$activityName = "app.verruckt.desktop.MainActivity"
Write-Host "`nLaunching $packageName/$activityName..." -ForegroundColor Yellow
$launchRes = & $adbBin -s $devSerial shell am start -n "${packageName}/${activityName}" 2>&1
Write-Host ($launchRes -join "`n")

# Verify PID
Start-Sleep -Seconds 2
$pidVal = (& $adbBin -s $devSerial shell pidof $packageName).Trim()
if ($pidVal) {
    Write-Host "`nApp running with PID: $pidVal" -ForegroundColor Green
} else {
    Write-Host "`n[WARN] App PID not detected immediately; checking ps..." -ForegroundColor Yellow
    & $adbBin -s $devSerial shell "ps -A | grep $packageName"
}

if ($FollowLogs -and $pidVal) {
    Write-Host "`nStreaming logcat for PID $pidVal (Press Ctrl+C to stop)..." -ForegroundColor Cyan
    & $adbBin -s $devSerial logcat --pid=$pidVal
}
