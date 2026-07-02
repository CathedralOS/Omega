# Refresh every sample's build/ with the CURRENT compiler, so you can walk into
# any samples/<name>/ and run .\build\omega-program.exe to see the latest state.
#
# Two steps, both load-bearing:
#   1. Rebuild apps/omega-cli. It is a SEPARATE cargo workspace: `cargo build
#      --workspace` and `cargo test` from the repo root rebuild the compiler
#      libs but never relink omega.exe, so it silently runs stale compiler code
#      until this step.
#   2. Compile every samples/*/main.omg IN PLACE (the CLI writes build/ next to
#      the source). The test harness never does this -- it builds into temp
#      dirs and deletes them, which is why sample folders are otherwise empty.
#
# Usage: powershell -ExecutionPolicy Bypass -File scripts\refresh_samples.ps1
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot

Write-Host "== rebuilding omega-cli (the CLI goes stale otherwise) =="
Push-Location (Join-Path $root "apps\omega-cli")
cargo build
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "CLI build FAILED"; exit 1 }
Pop-Location

$omega = Join-Path $root "target\debug\omega.exe"
$failures = @()
$built = 0
Get-ChildItem -Path (Join-Path $root "samples") -Recurse -Filter main.omg | ForEach-Object {
    Push-Location $_.DirectoryName
    $out = & $omega .\main.omg 2>&1
    if ($LASTEXITCODE -eq 0) {
        $built++
    } else {
        $first = ($out | Select-String -Pattern "error" | Select-Object -First 1)
        $failures += "$($_.DirectoryName) :: $first"
    }
    Pop-Location
}

Write-Host "== $built sample(s) built =="
if ($failures.Count -gt 0) {
    Write-Host "== $($failures.Count) FAILED =="
    $failures | ForEach-Object { Write-Host $_ }
    exit 1
}
