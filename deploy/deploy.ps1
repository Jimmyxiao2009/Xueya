param(
    [switch]$Build
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$deployDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $deployDir
try {
    if (-not (Test-Path -LiteralPath '.env')) {
        throw 'Missing deploy/.env. Copy deploy/.env.example to deploy/.env and fill it in.'
    }
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        throw 'docker is required.'
    }

    docker compose version *> $null
    if ($LASTEXITCODE -ne 0) {
        throw 'Docker Compose v2 (docker compose) is required.'
    }

    $httpPort = (Get-Content -LiteralPath '.env' | Where-Object { $_ -match '^ORBIT_HTTP_PORT=' } | Select-Object -Last 1)
    if ($httpPort) { $httpPort = $httpPort -replace '^ORBIT_HTTP_PORT=', '' } else { $httpPort = '1420' }

    $pullPolicy = @()
    if (Test-Path -LiteralPath 'orbit-demo-images.tar') {
        Write-Host 'Loading offline images from orbit-demo-images.tar...'
        docker load -i 'orbit-demo-images.tar'
        $pullPolicy = @('--pull', 'never')
    }

    if ($Build -or -not (Test-Path -LiteralPath 'orbit-demo-images.tar')) {
        Write-Host 'Building application images...'
        docker compose --env-file .env -f compose.yml build
        $pullPolicy = @()
    }

    Write-Host 'Validating Compose configuration...'
    docker compose --env-file .env -f compose.yml config --quiet
    if ($LASTEXITCODE -ne 0) { throw "docker compose config failed with exit code $LASTEXITCODE" }

    Write-Host 'Starting Orbit stack...'
    docker compose --env-file .env -f compose.yml up -d @pullPolicy --wait --wait-timeout 300
    if ($LASTEXITCODE -ne 0) { throw "docker compose up failed with exit code $LASTEXITCODE" }

    docker compose --env-file .env -f compose.yml ps
    Write-Host ''
    Write-Host "Orbit health endpoint: http://127.0.0.1:$httpPort/healthz"
    Write-Host 'Public URL must match ORBIT_PUBLIC_ORIGIN in deploy/.env.'
    Write-Host 'Put an HTTPS reverse proxy in front of the web service before exposing it.'
}
finally {
    Pop-Location
}
