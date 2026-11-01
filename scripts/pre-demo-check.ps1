param(
  [Parameter(Mandatory = $true)]
  [string]$CloudUrl,
  [string]$Email = $env:VERRUCKT_DEMO_EMAIL,
  [string]$Password = $env:VERRUCKT_DEMO_PASSWORD,
  [switch]$AllowLocalHttp
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$base = $CloudUrl.Trim().TrimEnd('/')
$isHttps = $base.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)
$isLocalHttp = $base -match '^http://(localhost|127\.0\.0\.1)(:\d+)?$'
if (-not $isHttps -and -not ($AllowLocalHttp -and $isLocalHttp)) {
  Write-Host "[FAIL] Cloud reachable" -ForegroundColor Red
  Write-Host "Reason: Cloud URL must use https:// (or pass -AllowLocalHttp for local development)."
  exit 1
}

$script:failed = $false
function Pass([string]$Name) {
  Write-Host "[PASS] $Name" -ForegroundColor Green
}
function Fail([string]$Name, [string]$Reason) {
  Write-Host "[FAIL] $Name" -ForegroundColor Red
  Write-Host "Reason: $Reason"
  $script:failed = $true
}

Write-Host "VERRUCKT PRE-DEMO CHECK"
Write-Host "Cloud: $base"
Write-Host ""

# 1-2: DNS/HTTP + health endpoint
try {
  $health = Invoke-RestMethod -Uri "$base/healthz" -Method Get -TimeoutSec 15
  if ($health.status -ne "ok") {
    throw "healthz returned status=$($health.status)"
  }
  Pass "Cloud reachable"
  Pass "Cloud health"
} catch {
  Fail "Cloud reachable" $_.Exception.Message
  Fail "Cloud health" $_.Exception.Message
  Write-Host ""
  Write-Host "NOT READY FOR DEMO"
  exit 1
}

if ([string]::IsNullOrWhiteSpace($Email) -or [string]::IsNullOrWhiteSpace($Password)) {
  Fail "Account login" "Set VERRUCKT_DEMO_EMAIL and VERRUCKT_DEMO_PASSWORD (or pass -Email/-Password)."
  Write-Host ""
  Write-Host "NOT READY FOR DEMO"
  exit 1
}

# 3: account login
$accessToken = $null
try {
  $loginBody = @{
    email = $Email
    password = $Password
    device = @{
      name = "Pre-demo check"
      platform = "windows"
      app_version = "0.1.0"
    }
  } | ConvertTo-Json -Depth 6
  $login = Invoke-RestMethod -Uri "$base/api/v1/auth/login" -Method Post -ContentType "application/json" -Body $loginBody -TimeoutSec 30
  $accessToken = $login.access_token
  if ([string]::IsNullOrWhiteSpace($accessToken)) {
    throw "login response did not contain an access token"
  }
  Pass "Account login"
} catch {
  Fail "Account login" $_.Exception.Message
  Write-Host ""
  Write-Host "NOT READY FOR DEMO"
  exit 1
}

$headers = @{ Authorization = "Bearer $accessToken" }

# 4-6: entitlement + quota
try {
  $entitlements = Invoke-RestMethod -Uri "$base/api/v1/me/entitlements" -Method Get -Headers $headers -TimeoutSec 30
  if (-not $entitlements.features.official_ai) {
    throw "official_ai entitlement is false"
  }
  Pass "Official AI entitlement"
  if (-not $entitlements.features.cloud_sync) {
    throw "cloud_sync entitlement is false"
  }
  Pass "Cloud Sync entitlement"

  $usage = Invoke-RestMethod -Uri "$base/api/v1/ai/usage" -Method Get -Headers $headers -TimeoutSec 30
  $limit = [int64]$usage.credits_limit
  $remaining = [int64]$usage.credits_remaining
  if ($limit -le 0 -or $remaining -le 0) {
    throw "AI quota is not available (limit=$limit remaining=$remaining)"
  }
  Pass "AI quota"
} catch {
  Fail "Entitlement/quota" $_.Exception.Message
  Write-Host ""
  Write-Host "NOT READY FOR DEMO"
  exit 1
}

# 7-8: real AI gateway request + upstream response
$structuredOk = $false
try {
  $chatBody = @{
    model = "verruckt-fast"
    messages = @(
      @{
        role = "system"
        content = "You are 学涯, a Socratic tutor. Return ONLY JSON with fields message (string), suggestedAction (string), masteryDelta (number)."
      },
      @{
        role = "user"
        content = "用一个问题开始讲解：为什么天空是蓝色的？"
      }
    )
    max_tokens = 256
    stream = $false
  } | ConvertTo-Json -Depth 8

  $chat = Invoke-RestMethod -Uri "$base/v1/chat/completions" -Method Post -Headers $headers -ContentType "application/json" -Body $chatBody -TimeoutSec 90
  $content = [string]$chat.choices[0].message.content
  if ([string]::IsNullOrWhiteSpace($content)) {
    throw "upstream returned an empty chat completion"
  }
  Pass "AI Gateway"
  Pass "Upstream model"

  $candidate = $content.Trim()
  if ($candidate.StartsWith('```')) {
    $candidate = $candidate.Trim('`').Trim()
  }
  try {
    $parsed = $candidate | ConvertFrom-Json
    if (-not [string]::IsNullOrWhiteSpace([string]$parsed.message) -and -not [string]::IsNullOrWhiteSpace([string]$parsed.suggestedAction)) {
      $structuredOk = $true
      Pass "Structured teaching response"
    }
  } catch {
    $structuredOk = $false
  }
} catch {
  Fail "AI Gateway/upstream" $_.Exception.Message
}

if (-not $structuredOk -and -not $script:failed) {
  Write-Host "[WARN] Structured teaching response" -ForegroundColor Yellow
  Write-Host "Reason: upstream returned plain text; the local TutorService will use its explicit fallback, but structured JSON is preferred."
}

# 9: sync protocol
try {
  Invoke-RestMethod -Uri "$base/api/v1/sync/pull?limit=1" -Method Get -Headers $headers -TimeoutSec 30 | Out-Null
  Pass "Cloud Sync protocol"
} catch {
  Fail "Cloud Sync protocol" $_.Exception.Message
}

Write-Host ""
if ($script:failed) {
  Write-Host "NOT READY FOR DEMO" -ForegroundColor Red
  exit 1
}
Write-Host "READY FOR DEMO" -ForegroundColor Green
