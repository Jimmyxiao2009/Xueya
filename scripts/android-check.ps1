# scripts/android-check.ps1
# Preflight environment and device check for Verruckt Android Bring-Up

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  VERRUCKT ANDROID PREFLIGHT CHECK" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$allPass = $true

# 1. Check JDK
Write-Host "`n[1/6] Checking JDK..." -ForegroundColor Yellow
$javaCmd = Get-Command java -ErrorAction SilentlyContinue
$javaBin = if ($javaCmd) { $javaCmd.Source } else { $null }
if (-not $javaBin -and $env:JAVA_HOME) {
    $candidate = Join-Path $env:JAVA_HOME "bin\java.exe"
    if (Test-Path $candidate) { $javaBin = $candidate }
}
if ($javaBin -and (Test-Path $javaBin)) {
    $javaVer = & $javaBin -version 2>&1 | Out-String
    Write-Host "  Found Java: $javaBin" -ForegroundColor Green
    Write-Host "  Version info: $($javaVer.Split("`n")[0].Trim())"
} else {
    Write-Host "  [FAIL] Java not found! Please set JAVA_HOME or add to PATH." -ForegroundColor Red
    $allPass = $false
}

# 2. Check Android SDK
Write-Host "`n[2/6] Checking Android SDK..." -ForegroundColor Yellow
$androidSdk = $env:ANDROID_HOME
if (-not $androidSdk) {
    $defaultSdk = "$env:LOCALAPPDATA\Android\Sdk"
    if (Test-Path $defaultSdk) {
        $androidSdk = $defaultSdk
        $env:ANDROID_HOME = $defaultSdk
    }
}
if ($androidSdk -and (Test-Path $androidSdk)) {
    Write-Host "  ANDROID_HOME: $androidSdk" -ForegroundColor Green
    $platforms = Get-ChildItem (Join-Path $androidSdk "platforms") -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }
    $buildTools = Get-ChildItem (Join-Path $androidSdk "build-tools") -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }
    Write-Host "  Platforms: $($platforms -join ', ')"
    Write-Host "  Build Tools: $($buildTools -join ', ')"
} else {
    Write-Host "  [FAIL] Android SDK not found! Please set ANDROID_HOME." -ForegroundColor Red
    $allPass = $false
}

# 3. Check NDK
Write-Host "`n[3/6] Checking Android NDK..." -ForegroundColor Yellow
$ndkPath = $env:NDK_HOME
if (-not $ndkPath -and $androidSdk) {
    $ndkDir = Join-Path $androidSdk "ndk"
    $installedNdks = Get-ChildItem $ndkDir -ErrorAction SilentlyContinue | Sort-Object Name -Descending
    if ($installedNdks) {
        $ndkPath = $installedNdks[0].FullName
        $env:NDK_HOME = $ndkPath
    }
}
if ($ndkPath -and (Test-Path $ndkPath)) {
    Write-Host "  NDK_HOME: $ndkPath" -ForegroundColor Green
    $clangBin = Join-Path $ndkPath "toolchains\llvm\prebuilt\windows-x86_64\bin\clang.exe"
    if (Test-Path $clangBin) {
        Write-Host "  Clang toolchain: OK ($clangBin)" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] NDK Clang not found at $clangBin" -ForegroundColor Red
        $allPass = $false
    }
} else {
    Write-Host "  [FAIL] Android NDK not found! Please set NDK_HOME." -ForegroundColor Red
    $allPass = $false
}

# 4. Check Rust Target
Write-Host "`n[4/6] Checking Rust toolchain..." -ForegroundColor Yellow
$rustcVer = & rustc --version 2>&1
Write-Host "  Rustc: $rustcVer"
$installedTargets = & rustup target list --installed 2>&1
if ($installedTargets -contains "aarch64-linux-android") {
    Write-Host "  Target aarch64-linux-android: OK" -ForegroundColor Green
} else {
    Write-Host "  [FAIL] Target aarch64-linux-android not installed! Run: rustup target add aarch64-linux-android" -ForegroundColor Red
    $allPass = $false
}

# 5. Check Local Network IP
Write-Host "`n[5/6] Checking Host LAN IP..." -ForegroundColor Yellow
$lanIps = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { 
    $_.InterfaceAlias -notmatch "vEthernet|Loopback|WSL" -and $_.IPAddress -notmatch "^127\.|^169\.254\."
}
if ($lanIps) {
    foreach ($ip in $lanIps) {
        Write-Host "  Adapter: $($ip.InterfaceAlias) -> IP: $($ip.IPAddress)" -ForegroundColor Green
    }
} else {
    Write-Host "  [WARN] No LAN IPv4 address detected!" -ForegroundColor Yellow
}

# 6. Check ADB and Physical Devices
Write-Host "`n[6/6] Checking ADB & Physical Devices..." -ForegroundColor Yellow
$adbCmd = Get-Command adb -ErrorAction SilentlyContinue
$adbBin = if ($adbCmd) { $adbCmd.Source } else { $null }
if (-not $adbBin -and $androidSdk) {
    $candidate = Join-Path $androidSdk "platform-tools\adb.exe"
    if (Test-Path $candidate) { $adbBin = $candidate }
}
if ($adbBin) {
    $devices = & $adbBin devices -l 2>&1
    Write-Host ($devices -join "`n")
    $connected = $devices | Where-Object { $_ -match "^\s*([a-zA-Z0-9\._\-:]+)\s+device\b" }
    if ($connected) {
        Write-Host "`n  Physical Device Connected!" -ForegroundColor Green
        $devSerial = ($connected[0] -split '\s+')[0]
        $manufacturer = (& $adbBin -s $devSerial shell getprop ro.product.manufacturer).Trim()
        $model = (& $adbBin -s $devSerial shell getprop ro.product.model).Trim()
        $androidVer = (& $adbBin -s $devSerial shell getprop ro.build.version.release).Trim()
        $sdkVer = (& $adbBin -s $devSerial shell getprop ro.build.version.sdk).Trim()
        $abi = (& $adbBin -s $devSerial shell getprop ro.product.cpu.abi).Trim()
        Write-Host "    Manufacturer: $manufacturer"
        Write-Host "    Model:        $model"
        Write-Host "    Android:      $androidVer (SDK $sdkVer)"
        Write-Host "    ABI:          $abi"
    } else {
        Write-Host "`n  [NOTICE] No authorized device currently attached." -ForegroundColor Yellow
        Write-Host "  Please connect your Android phone via USB, enable USB debugging, and accept the prompt." -ForegroundColor Yellow
    }
} else {
    Write-Host "  [FAIL] adb.exe not found in PATH or Android SDK platform-tools." -ForegroundColor Red
    $allPass = $false
}

Write-Host "`n========================================" -ForegroundColor Cyan
if ($allPass) {
    Write-Host "  PREFLIGHT RESULT: READY" -ForegroundColor Green
    exit 0
} else {
    Write-Host "  PREFLIGHT RESULT: MISSING DEPENDENCIES" -ForegroundColor Red
    exit 1
}
