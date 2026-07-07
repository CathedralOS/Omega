# Rebuild the CLI (it is a SEPARATE workspace -- `cargo build --workspace` never
# relinks it, so it silently runs stale compiler code), then compile every
# sample in place, in parallel, via the cross-platform `omega refresh-samples`
# subcommand. Result: every samples/<domain>/<name>/build/omega-program.exe is current.
$root = Split-Path -Parent $PSScriptRoot
Push-Location (Join-Path $root "apps\omega-cli")
cargo build
$ok = $LASTEXITCODE -eq 0
Pop-Location
if (-not $ok) { exit 1 }
Set-Location $root
& (Join-Path $root "target\debug\omega.exe") refresh-samples samples
exit $LASTEXITCODE
