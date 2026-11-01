[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Server,

    [string]$User = 'root',
    [int]$Port = 22,
    [string]$IdentityFile,
    [string]$RemoteDir = '/opt/orbit',

    [switch]$SkipUpload,
    [switch]$RemoteSudo,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

foreach ($tool in @('ssh', 'scp')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool is required. Install the Windows OpenSSH Client feature."
    }
}

if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'compose.yml')) {
    # Layout used inside scripts/package-release.ps1 server bundle.
    $deployDir = $PSScriptRoot
    $tar = Join-Path $PSScriptRoot 'orbit-demo-images.tar'
} else {
    # Repository layout.
    $root = Split-Path -Parent $PSScriptRoot
    $deployDir = Join-Path $root 'deploy'
    $tar = Join-Path $root '.orbit-server\orbit-demo-images.tar'
}
$envFile = Join-Path $deployDir '.env'
$composeFile = Join-Path $deployDir 'compose.yml'
$deployScript = Join-Path $deployDir 'deploy.sh'
$preflightScript = Join-Path $deployDir 'preflight.sh'
$dockerBootstrapScript = Join-Path $deployDir 'install-docker-ubuntu.sh'
$backupScript = Join-Path $deployDir 'backup.sh'
$restoreScript = Join-Path $deployDir 'restore.sh'
$readme = Join-Path $deployDir 'README.md'
$caddyExample = Join-Path $deployDir 'Caddyfile.example'

if (-not (Test-Path -LiteralPath $envFile)) {
    throw "Missing $envFile. Copy deploy/.env.example to deploy/.env and fill it in first."
}
if (-not (Test-Path -LiteralPath $tar)) {
    throw "Missing $tar. Build/save the offline image bundle first."
}

$sshBase = @('-p', "$Port", '-o', 'ConnectTimeout=15')
$scpBase = @('-P', "$Port", '-o', 'ConnectTimeout=15')
if ($IdentityFile) {
    if (-not (Test-Path -LiteralPath $IdentityFile)) {
        throw "Identity file not found: $IdentityFile"
    }
    $sshBase += @('-i', $IdentityFile)
    $scpBase += @('-i', $IdentityFile)
}

$target = "$User@$Server"
$remotePrefix = if ($RemoteSudo) { 'sudo ' } else { '' }

function Invoke-Step {
    param(
        [Parameter(Mandatory)] [string]$Description,
        [Parameter(Mandatory)] [scriptblock]$Action
    )
    Write-Host "==> $Description"
    if ($DryRun) {
        return
    }
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

$quotedDir = $RemoteDir
Invoke-Step "Create remote directory $RemoteDir" {
    & ssh @sshBase $target "mkdir -p '$quotedDir'"
}

if (-not $SkipUpload) {
    Invoke-Step 'Upload deployment files' {
        & scp @scpBase $composeFile $deployScript $preflightScript $dockerBootstrapScript $backupScript $restoreScript $envFile $readme $caddyExample "${target}:$RemoteDir/"
    }
    Invoke-Step 'Upload offline images (this may take a while)' {
        & scp @scpBase $tar "${target}:$RemoteDir/orbit-demo-images.tar"
    }
}

Invoke-Step 'Run offline deployment on the server' {
    & ssh @sshBase $target "cd '$quotedDir' && chmod +x deploy.sh && ${remotePrefix}sh deploy.sh"
}

Write-Host ''
Write-Host 'Remote deployment command completed.'
Write-Host "Check on the server: cd $RemoteDir && docker compose --env-file .env -f compose.yml ps"
