# Runs the real desktop -> Verruckt Cloud -> mock OpenAI upstream E2E test.
#
# It starts the Docker Compose stack, creates a fresh pro account, then runs
# the ignored `cloud_live` integration test with that account.
#
# Usage:
#   pwsh -File scripts/e2e-cloud.ps1
param(
    [string]$Email = "e2e-$([guid]::NewGuid().ToString('N').Substring(0, 8))@example.com",
    [string]$Password = $env:VERRUCKT_E2E_PASSWORD,
    [string]$CloudUrl = "http://127.0.0.1:8080"
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Password)) {
  $Password = "e2e-" + [guid]::NewGuid().ToString("N")
}


Write-Host "Starting Verruckt Cloud compose stack..."
Push-Location (Join-Path $PSScriptRoot "..\cloud")
try {
    docker compose up -d --build
} finally {
    Pop-Location
}

Write-Host "Waiting for Verruckt Cloud health..."
$healthy = $false
for ($attempt = 0; $attempt -lt 60; $attempt++) {
    try {
        $response = Invoke-RestMethod -Uri "$CloudUrl/healthz" -TimeoutSec 3
        if ($response.status -eq "ok") {
            $healthy = $true
            break
        }
    } catch {
        Start-Sleep -Seconds 2
    }
}
if (-not $healthy) {
    throw "Verruckt Cloud did not become healthy at $CloudUrl"
}

Write-Host "Registering E2E account $Email..."
$body = @{ email = $Email; password = $Password } | ConvertTo-Json
Invoke-RestMethod -Uri "$CloudUrl/api/v1/auth/register" -Method Post `
    -ContentType "application/json" -Body $body | Out-Null

Write-Host "Upgrading E2E account to pro..."
Push-Location (Join-Path $PSScriptRoot "..\cloud")
try {
    docker compose exec -T postgres psql -U verruckt -d verruckt -c `
        "UPDATE subscriptions SET plan_id='pro', status='active', updated_at=now() WHERE user_id=(SELECT id FROM users WHERE email_normalized='$Email');"
} finally {
    Pop-Location
}

$env:VERRUCKT_CLOUD_URL = $CloudUrl
$env:VERRUCKT_E2E_EMAIL = $Email
$env:VERRUCKT_E2E_PASSWORD = $Password

Write-Host "Running desktop cloud_live integration test..."
Push-Location (Join-Path $PSScriptRoot "..\src-tauri")
try {
    cargo test --test cloud_live -- --ignored --nocapture --test-threads=1
    $exitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($exitCode -ne 0) {
    throw "cloud_live integration test failed with exit code $exitCode"
}
Write-Host "Cloud E2E passed for $Email"
