# scripts/android-dev-lan.ps1
# Start Tauri Android Dev server bound to PC LAN IP for physical device Wi-Fi access

param(
    [string]$LanIp = ""
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  VERRUCKT ANDROID LAN DEV LAUNCHER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# Detect LAN IP if not provided
if (-not $LanIp) {
    # Prioritize Wi-Fi, WLAN, or 以太网 physical adapters
    $preferred = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { 
        $_.InterfaceAlias -match "以太网|Wi-Fi|WLAN|Ethernet" -and 
        $_.InterfaceAlias -notmatch "VMware|Virtual|vEthernet|Loopback|WSL|Meta|Tailscale" -and 
        $_.IPAddress -notmatch "^127\.|^169\.254\."
    }
    if ($preferred) {
        $LanIp = $preferred[0].IPAddress
    } else {
        $fallback = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { 
            $_.InterfaceAlias -notmatch "VMware|Virtual|vEthernet|Loopback|WSL|Meta|Tailscale" -and 
            $_.IPAddress -notmatch "^127\.|^169\.254\."
        }
        if ($fallback) {
            $LanIp = $fallback[0].IPAddress
        } else {
            Write-Error "No valid LAN IP address found! Please pass -LanIp <your-lan-ip>"
            exit 1
        }
    }
}

Write-Host "Target LAN IP: $LanIp" -ForegroundColor Green
Write-Host "Vite / Tauri Dev Server will be reachable at: http://${LanIp}:1420" -ForegroundColor Green
Write-Host "Vite HMR WebSocket will be reachable at: ws://${LanIp}:1421" -ForegroundColor Green

# Set required Android / Rust build environment
$env:CARGO_BUILD_JOBS = "4"
$env:CARGO_TARGET_DIR = "C:\verruckt-target"
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = "C:\Program Files\Microsoft\jdk-17.0.10.7-hotspot" }
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk" }
if (-not $env:NDK_HOME) { $env:NDK_HOME = "$env:ANDROID_HOME\ndk\27.0.12077973" }
$env:TAURI_DEV_HOST = $LanIp

Write-Host "`nLaunching Tauri Android dev on host $LanIp..." -ForegroundColor Yellow
npm run tauri android dev -- --host $LanIp
