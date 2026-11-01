param(
  [Alias("CloudBaseUrl")]
  [Parameter(Mandatory = $true)]
  [string]$CloudUrl,
  [ValidateSet("Release", "Debug")]
  [string]$Profile = "Release",
  [switch]$SkipTests,
  [switch]$NoBundle,
  [string]$Bundles = "nsis",
  [switch]$AllowLocalHttp
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail([string]$Message) {
  Write-Host "[FAIL] $Message" -ForegroundColor Red
  exit 1
}

$value = $CloudUrl.Trim().TrimEnd('/')
if ([string]::IsNullOrWhiteSpace($value)) {
  Fail "Cloud URL is required."
}

$uri = $null
if (-not [Uri]::TryCreate($value, [UriKind]::Absolute, [ref]$uri)) {
  Fail "Cloud URL is not a valid absolute URL: $value"
}
if ($uri.Scheme -notin @("http", "https")) {
  Fail "Cloud URL must use http or https: $value"
}
if ([string]::IsNullOrWhiteSpace($uri.Host) -or $uri.Host.Contains(" ")) {
  Fail "Cloud URL has an invalid host: $value"
}

$isHttps = $uri.Scheme -eq "https"
$isLocalHttp = $uri.Scheme -eq "http" -and $uri.Host -in @("localhost", "127.0.0.1")
if (-not $isHttps -and -not ($AllowLocalHttp -and $isLocalHttp)) {
  Fail "Production Cloud URL must use https://. Use -AllowLocalHttp only for local development endpoints."
}

$env:VERRUCKT_DEFAULT_CLOUD_URL = $value

Write-Host ""
Write-Host "Orbit / 学涯 production build"
Write-Host "Cloud endpoint: $value"
Write-Host "Mode: $Profile"
Write-Host "Tests: $(-not $SkipTests)"
Write-Host "Bundle: $(-not $NoBundle)"
Write-Host ""

if (-not $SkipTests) {
  Write-Host "==> npm run typecheck"
  npm run typecheck
  if ($LASTEXITCODE -ne 0) { Fail "typecheck failed" }

  Write-Host "==> npm run lint"
  npm run lint
  if ($LASTEXITCODE -ne 0) { Fail "lint failed" }
}

Write-Host "==> npm run build"
npm run build
if ($LASTEXITCODE -ne 0) { Fail "frontend build failed" }

$tauriArgs = @("tauri", "build")
if ($Profile -eq "Debug") { $tauriArgs += "--debug" }
if ($NoBundle) {
  $tauriArgs += "--no-bundle"
} elseif (-not [string]::IsNullOrWhiteSpace($Bundles)) {
  $tauriArgs += @("--bundles", $Bundles)
}

Write-Host "==> npx $($tauriArgs -join ' ')"
npx @tauriArgs
if ($LASTEXITCODE -ne 0) { Fail "Tauri build failed" }

$targetDir = Join-Path $root "src-tauri\target"
$profileDir = if ($Profile -eq "Debug") { "debug" } else { "release" }
$profilePath = Join-Path $targetDir $profileDir

$executable = Get-ChildItem -Path $profilePath -Filter "*.exe" -File -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notmatch "setup|bundle|wix" } |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

$installer = Get-ChildItem -Path (Join-Path $profilePath "bundle") -Recurse -Include "*.exe", "*.msi" -File -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

Write-Host ""
Write-Host "Build artifacts:"
if ($installer) {
  Write-Host "Installer: $($installer.FullName)"
} else {
  Write-Host "Installer: not produced (use without -NoBundle)"
}
if ($executable) {
  Write-Host "Executable: $($executable.FullName)"
} else {
  Write-Host "Executable: not found"
}
Write-Host "Cloud endpoint embedded: $value"
Write-Host "BUILD PASS"
