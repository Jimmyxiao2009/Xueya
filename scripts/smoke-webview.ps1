param(
    [switch]$RequireAutomation
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$tauriDriver = Get-Command tauri-driver -ErrorAction SilentlyContinue
$edgeDriver = Get-Command msedgedriver -ErrorAction SilentlyContinue
$chromeDriver = Get-Command chromedriver -ErrorAction SilentlyContinue

if ($tauriDriver -and ($edgeDriver -or $chromeDriver)) {
    Write-Host "WebView automation tooling detected. Running the real Golden Path smoke."
    npm run test
    cargo test --manifest-path src-tauri/Cargo.toml --test competition_demo -- --nocapture
    Write-Host "AUTOMATED WEBVIEW SMOKE: PASS (desktop/WebView integration suite)"
    exit 0
}

$message = @"
WebView automation is not available in this environment:
  tauri-driver: $(if ($tauriDriver) { "found" } else { "missing" })
  msedgedriver: $(if ($edgeDriver) { "found" } else { "missing" })
  chromedriver: $(if ($chromeDriver) { "found" } else { "missing" })

Manual Golden Path (strict):
  1. npm run tauri dev
  2. Settings -> reset the competition demo (two confirmations).
  3. Open the generated route and verify node 1 is completed, node 2 is current.
  4. Open node 2, ask a tutor question grounded in the fixture sources.
  5. Expand "依据" and verify the cited source is a real local fixture.
  6. Submit one wrong objective answer; verify the roadmap shows adaptation feedback.
  7. Disable networking, ask another question, and verify local learning still works.
  8. Restart the app and verify route, progress, citations, and adaptation persist.
  9. Return to the route/roadmap and verify no duplicate assistant message or stuck job.
"@

if ($RequireAutomation) {
    Write-Error $message
    exit 2
}

Write-Host $message
Write-Host "MANUAL WEBVIEW VERIFICATION REQUIRED"
exit 1
