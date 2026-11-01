[CmdletBinding()]
param(
    [string]$WindowsBundle,
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [switch]$WarnOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $WindowsBundle) {
    $WindowsBundle = Get-ChildItem -LiteralPath (Join-Path $Root '.orbit-server\release') -Filter 'orbit-windows-*.zip' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $WindowsBundle -or -not (Test-Path -LiteralPath $WindowsBundle)) {
    throw "Windows release bundle not found: $WindowsBundle"
}

Write-Host "Checking Windows bundle: $WindowsBundle"

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($WindowsBundle)
try {
    $entries = @($zip.Entries | ForEach-Object { $_.FullName })
}
finally {
    $zip.Dispose()
}

$requiredPatterns = @(
    '*x64-setup.exe',
    '*x64_en-US.msi',
    '*_x64.msix',
    'sign-windows.ps1',
    'STORE.md',
    'PRIVACY.md',
    'TERMS.md',
    'SUPPORT.md',
    'Orbit_300x300.png',
    'screenshots/*.png',
    'store-site/index.html',
    'store-site/privacy.html',
    'store-site/support.html'
)

$missing = @()
foreach ($pattern in $requiredPatterns) {
    if (-not ($entries | Where-Object { $_ -like $pattern })) {
        $missing += $pattern
    }
}
if ($missing.Count -gt 0) {
    $message = "Windows bundle is missing required entries: $($missing -join ', ')"
    if ($WarnOnly) { Write-Warning $message } else { throw $message }
}

$complianceFiles = @(
    (Join-Path $Root 'docs\PRIVACY.md'),
    (Join-Path $Root 'docs\TERMS.md'),
    (Join-Path $Root 'docs\SUPPORT.md'),
    (Join-Path $Root 'store\site\index.html'),
    (Join-Path $Root 'store\site\privacy.html'),
    (Join-Path $Root 'store\site\terms.html'),
    (Join-Path $Root 'store\site\support.html')
) | Where-Object { Test-Path -LiteralPath $_ }

$placeholderPatterns = @(
    '\[支持邮箱\]',
    '\[运营主体\]',
    '\[隐私政策 URL\]',
    '\[支持 URL\]',
    '\[适用法律/地区\]',
    '\[Support Email\]',
    '\[Operator\]',
    '\[Privacy Policy URL\]',
    '\[Support URL\]'
)

$findings = @()
foreach ($file in $complianceFiles) {
    $content = Get-Content -LiteralPath $file -Raw
    foreach ($pattern in $placeholderPatterns) {
        if ($content -match $pattern) {
            $findings += "$file -> $pattern"
        }
    }
}

if ($findings.Count -gt 0) {
    Write-Host ''
    Write-Host 'Store compliance placeholders still present:'
    $findings | ForEach-Object { Write-Host "  $_" }
    Write-Host ''
    $message = "Found $($findings.Count) placeholder occurrence(s). Replace them before Partner Center submission."
    if ($WarnOnly) {
        Write-Warning $message
    } else {
        throw $message
    }
} else {
    Write-Host 'No store compliance placeholders found.'
}

Write-Host 'Store release check passed.'
