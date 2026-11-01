[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Pfx,

    [string]$PfxPassword = $env:ORBIT_SIGN_PFX_PASSWORD,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [string[]]$Artifact,
    [string]$ArtifactList,
    [switch]$SkipTimestamp,
    [switch]$SkipVerify
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not (Test-Path -LiteralPath $Pfx)) {
    throw "PFX not found: $Pfx"
}
if (-not $PfxPassword) {
    throw 'PfxPassword is required. Pass -PfxPassword or set ORBIT_SIGN_PFX_PASSWORD.'
}

$root = Split-Path -Parent $PSScriptRoot
if ($ArtifactList) {
    $Artifact = @($ArtifactList -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}
if (-not $Artifact -or $Artifact.Count -eq 0) {
    $nsis = Get-ChildItem -LiteralPath (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter '*x64-setup.exe' -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $msi = Get-ChildItem -LiteralPath (Join-Path $root 'src-tauri\target\release\bundle\msi') -Filter '*x64_en-US.msi' -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $msix = Get-ChildItem -LiteralPath (Join-Path $root 'src-tauri\target\release\bundle\msix') -Filter '*_x64.msix' -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $found = @()
    if ($nsis) { $found += $nsis.FullName }
    if ($msi) { $found += $msi.FullName }
    if ($msix) { $found += $msix.FullName }
    $Artifact = $found
}

$missing = @($Artifact | Where-Object { -not (Test-Path -LiteralPath $_) })
if ($missing.Count -gt 0) {
    throw "Missing artifacts: $($missing -join ', ')"
}

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signtool) {
    throw 'signtool.exe not found. Install the Windows 10/11 SDK.'
}
$signtoolPath = $signtool.FullName

foreach ($file in $Artifact) {
    Write-Host "Signing $file"
    $signArgs = @('sign', '/fd', 'SHA256', '/f', $Pfx)
    if ($PfxPassword) {
        $signArgs += @('/p', $PfxPassword)
    }
    if (-not $SkipTimestamp) {
        $signArgs += @('/tr', $TimestampUrl, '/td', 'SHA256')
    }
    $signArgs += $file
    & $signtoolPath @signArgs
    if ($LASTEXITCODE -ne 0) {
        throw "signtool sign failed for $file with exit code $LASTEXITCODE"
    }
}

foreach ($file in $Artifact) {
    $signature = Get-AuthenticodeSignature -LiteralPath $file
    Write-Host "Signature: $([System.IO.Path]::GetFileName($file)) -> $($signature.Status)"
    if ($signature.Status -eq 'NotSigned') {
        throw "Artifact is still unsigned: $file"
    }
    if (-not $SkipVerify) {
        & $signtoolPath verify /pa /v $file
        if ($LASTEXITCODE -ne 0) {
            throw "signtool verify failed for $file with exit code $LASTEXITCODE"
        }
    }
}

Write-Host 'Windows signing completed.'
