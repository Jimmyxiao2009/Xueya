[CmdletBinding()]
param(
    [string]$BaseUrl = 'http://127.0.0.1:1420',
    [string]$Email,
    [string]$Password = $env:VERRUCKT_SMOKE_PASSWORD,
    [switch]$CreateRoute,
    [int]$TimeoutSeconds = 300
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$base = $BaseUrl.TrimEnd('/')
if (-not $Email) {
    $Email = "smoke.$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())@example.com"
}

if ([string]::IsNullOrWhiteSpace($Password)) {
    $Password = "smoke-" + [guid]::NewGuid().ToString("N")
}


Write-Host "==> Health $base/healthz"
$health = Invoke-RestMethod -Uri "$base/healthz" -Method Get -TimeoutSec 15
if ($health.status -ne 'ok') {
    throw "healthz returned status=$($health.status)"
}
if (-not $health.aiConfigured) {
    throw 'healthz says aiConfigured=false'
}
Write-Host "healthz ok: aiConfigured=$($health.aiConfigured) dataMode=$($health.dataMode)"

$session = New-Object Microsoft.PowerShell.Commands.WebRequestSession

function Invoke-Orbit {
    param(
        [Parameter(Mandatory)] [string]$Command,
        [Parameter(Mandatory)] [hashtable]$Arguments
    )
    $payload = @{ command = $Command; args = $Arguments } | ConvertTo-Json -Depth 10 -Compress
    return Invoke-RestMethod `
        -Uri "$base/api/invoke" `
        -Method Post `
        -ContentType 'application/json' `
        -Headers @{ 'X-Orbit-Client' = 'web' } `
        -Body $payload `
        -WebSession $session `
        -TimeoutSec 30
}

Write-Host "==> Register smoke account $Email"
$register = Invoke-Orbit -Command 'cloud_register' -Arguments @{ input = @{ email = $Email; password = $Password } }
if (-not $register.loggedIn) {
    throw 'cloud_register did not return loggedIn=true'
}

Write-Host '==> Verify session with cloud_get_me'
$me = Invoke-Orbit -Command 'cloud_get_me' -Arguments @{}
if ($me.email -ne $Email) {
    throw "cloud_get_me returned unexpected email"
}

if ($CreateRoute) {
    Write-Host '==> Create AI route (this can take a few minutes)'
    $job = Invoke-Orbit -Command 'course_create' -Arguments @{
        input = @{
            topic = 'Production deployment smoke test'
            goal = 'Verify that the deployed service can call the real AI gateway and create a route'
            experienceLevel = 'beginner'
            desiredDepth = 'overview'
            sourceIds = @()
        }
    }
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        Start-Sleep -Seconds 10
        $job = Invoke-Orbit -Command 'job_get' -Arguments @{ id = $job.id }
        Write-Host ("job {0} status={1} progress={2}" -f $job.id, $job.status, $job.progress)
    } while ($job.status -in @('pending', 'running') -and (Get-Date) -lt $deadline)

    if ($job.status -ne 'completed') {
        throw "AI route job ended with status=$($job.status) error=$($job.errorJson)"
    }
    $result = $job.resultJson | ConvertFrom-Json
    $course = Invoke-Orbit -Command 'course_get' -Arguments @{ id = $result.courseId }
    $nodeCount = @($course.nodes).Count
    Write-Host "route ok: id=$($result.courseId) title=$($course.course.title) nodes=$nodeCount"
}

Write-Host ''
Write-Host 'Production smoke test passed.'
