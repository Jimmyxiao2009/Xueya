param([switch]$NoBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$stateDir = Join-Path $projectRoot '.orbit-server'
New-Item -ItemType Directory -Force $stateDir | Out-Null
$configFile = Join-Path $projectRoot '.env.local'
if (!(Test-Path -LiteralPath $configFile)) {
  $dbPassword = [Convert]::ToHexString([System.Security.Cryptography.RandomNumberGenerator]::GetBytes(24)).ToLower()
  $jwtSecret = [Convert]::ToHexString([System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLower()
  $adminToken = [Convert]::ToHexString([System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLower()
  @(
    "ORBIT_DB_PASSWORD=$dbPassword", "VERRUCKT_ACCESS_TOKEN_SECRET=$jwtSecret", "ORBIT_ADMIN_TOKEN=$adminToken",
    'ORBIT_AI_BASE_URL=', 'ORBIT_AI_API_KEY=', 'ORBIT_AI_MODEL=', 'ORBIT_EMBEDDING_MODEL=text-embedding-3-small'
  ) | Set-Content -LiteralPath $configFile -Encoding utf8
}
Get-Content -LiteralPath $configFile | ForEach-Object {
  if ($_ -match '^([A-Z][A-Z0-9_]*)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
}
$postgresEnv = Join-Path $stateDir 'postgres.env'
@('POSTGRES_USER=orbit', 'POSTGRES_DB=orbit', "POSTGRES_PASSWORD=$env:ORBIT_DB_PASSWORD") | Set-Content -LiteralPath $postgresEnv -Encoding utf8
$existing = docker ps -a --filter 'name=^orbit-demo-postgres$' --format '{{.Names}}'
if (!$existing) { docker run -d --name orbit-demo-postgres --restart unless-stopped --env-file $postgresEnv -p 127.0.0.1:55440:5432 -v orbit-demo-postgres:/var/lib/postgresql/data postgres:16-alpine | Out-Null }
else { docker start orbit-demo-postgres | Out-Null }
if ($LASTEXITCODE -ne 0) { throw 'PostgreSQL could not start' }
if (!$NoBuild) {
  cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server --bin orbit-server -j 1
  if ($LASTEXITCODE -ne 0) { throw 'Learning server build failed' }
  cargo build --manifest-path cloud/Cargo.toml --bin verruckt-cloud -j 1
  if ($LASTEXITCODE -ne 0) { throw 'Account service build failed' }
}
$env:DATABASE_URL = "postgres://orbit:$env:ORBIT_DB_PASSWORD@127.0.0.1:55440/orbit"
$env:VERRUCKT_ENV = 'development'
$env:VERRUCKT_BIND = '127.0.0.1'
$env:VERRUCKT_PORT = '1431'
$env:VERRUCKT_CLOUD_URL = 'http://127.0.0.1:1431'
$env:ORBIT_DATA_DIR = $stateDir
$env:ORBIT_PUBLIC_ORIGIN = 'http://127.0.0.1:1420'
$env:ORBIT_REQUIRE_AI = '1'
$env:ORBIT_BIND = '127.0.0.1:1430'
function Start-DemoService($exe, $port, $name) {
  $listener = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue
  if ($listener) {
    $running = Get-Process -Id $listener[0].OwningProcess -ErrorAction Stop
    if ($running.Path -ne $exe) { throw "Port $port belongs to another service; Orbit will not replace it." }
    Write-Host "$name already listening on $port"; return
  }
  $proc = Start-Process -FilePath $exe -WorkingDirectory $projectRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $stateDir "$name.log") -RedirectStandardError (Join-Path $stateDir "$name-error.log") -PassThru
  $proc.Id | Set-Content -LiteralPath (Join-Path $stateDir "$name.pid")
}
Start-DemoService (Join-Path $projectRoot 'cloud/target/debug/verruckt-cloud.exe') 1431 'cloud'
Start-DemoService (Join-Path $projectRoot 'src-tauri/target/debug/orbit-server.exe') 1430 'learning'
if (!(Get-NetTCPConnection -State Listen -LocalPort 1420 -ErrorAction SilentlyContinue)) {
  $vite = Join-Path $projectRoot 'node_modules/vite/bin/vite.js'
  Start-Process -FilePath (Get-Command node).Source -ArgumentList @($vite,'--host','127.0.0.1') -WorkingDirectory $projectRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $stateDir 'vite.log') -RedirectStandardError (Join-Path $stateDir 'vite-error.log') | Out-Null
}
Write-Host 'Orbit: http://127.0.0.1:1420/'
Write-Host 'Admin: http://127.0.0.1:1420/admin.html (ORBIT_ADMIN_TOKEN in .env.local)'
if (!$env:ORBIT_AI_API_KEY) { Write-Host 'Real AI is not configured yet. Fill ORBIT_AI_* in .env.local, then restart the learning service.' }
