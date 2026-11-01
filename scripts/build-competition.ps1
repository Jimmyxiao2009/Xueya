param(
  [string]$CloudUrl = $env:VERRUCKT_COMPETITION_CLOUD_URL,
  [switch]$SkipTests,
  [switch]$NoBundle,
  [switch]$AllowLocalHttp
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($CloudUrl)) {
  throw "Provide -CloudUrl or set VERRUCKT_COMPETITION_CLOUD_URL."
}

$build = Join-Path $PSScriptRoot "build-with-cloud.ps1"
& $build -CloudUrl $CloudUrl -Profile Release -SkipTests:$SkipTests -NoBundle:$NoBundle -AllowLocalHttp:$AllowLocalHttp
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
