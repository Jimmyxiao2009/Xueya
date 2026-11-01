param(
  [ValidateSet("LocalCloud", "Remote")]
  [string]$Mode = "LocalCloud",
  [string]$CloudUrl = $env:VERRUCKT_COMPETITION_CLOUD_URL,
  [string]$Email = $env:VERRUCKT_DEMO_EMAIL,
  [string]$Password = $env:VERRUCKT_DEMO_PASSWORD
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

function Fail([string]$Message) {
  Write-Host "[FAIL] $Message" -ForegroundColor Red
  exit 1
}

if ($Mode -eq "LocalCloud") {
  Write-Host "DEMO SMOKE: local Docker Compose Verruckt Cloud + mock upstream"
  $script = Join-Path $PSScriptRoot "e2e-cloud.ps1"
  & $script
  if ($LASTEXITCODE -ne 0) { Fail "local cloud smoke failed" }
  Write-Host "DEMO SMOKE PASS (LocalCloud)" -ForegroundColor Green
  exit 0
}

if ([string]::IsNullOrWhiteSpace($CloudUrl)) {
  Fail "Remote mode requires -CloudUrl or VERRUCKT_COMPETITION_CLOUD_URL."
}
if ([string]::IsNullOrWhiteSpace($Email) -or [string]::IsNullOrWhiteSpace($Password)) {
  Fail "Remote mode requires VERRUCKT_DEMO_EMAIL and VERRUCKT_DEMO_PASSWORD."
}

$uri = $null
if (-not [Uri]::TryCreate($CloudUrl.Trim().TrimEnd('/'), [UriKind]::Absolute, [ref]$uri)) {
  Fail "Remote CloudUrl is not a valid absolute URL."
}
if ($uri.Scheme -ne "https" -or $uri.Host -in @("localhost", "127.0.0.1")) {
  Fail "Remote mode requires a non-localhost https:// Cloud URL. Use LocalCloud mode for local compose."
}

Write-Host "DEMO SMOKE: REAL REMOTE CLOUD"
Write-Host "Cloud: $($uri.GetLeftPart([System.UriPartial]::Authority))"

$check = Join-Path $PSScriptRoot "pre-demo-check.ps1"
& $check -CloudUrl $CloudUrl -Email $Email -Password $Password
if ($LASTEXITCODE -ne 0) { Fail "pre-demo-check failed" }

$env:VERRUCKT_CLOUD_URL = $CloudUrl.Trim().TrimEnd('/')
$env:VERRUCKT_E2E_EMAIL = $Email
$env:VERRUCKT_E2E_PASSWORD = $Password

Push-Location (Join-Path $root "src-tauri")
try {
  cargo test --test cloud_live -- --ignored --nocapture --test-threads=1
  if ($LASTEXITCODE -ne 0) { Fail "remote cloud_live smoke failed" }
} finally {
  Pop-Location
}

Write-Host "DEMO SMOKE PASS (Remote)" -ForegroundColor Green
