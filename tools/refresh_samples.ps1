# Rebuild the Rust development CLI, then compile every sample in place, in parallel,
# via the cross-platform `omega refresh-samples` subcommand. Result: every
# samples/<domain>/<name>/build/omega-program.exe is current.
$mbxCommand = Get-Command mbx -ErrorAction SilentlyContinue
if (-not $mbxCommand) {
    Write-Error "mbx 1.7.0 or newer is required; direct Cargo fallback is forbidden"
    exit 1
}
$mbxVersionText = & mbx --version
$mbxVersion = [version](($mbxVersionText -split ' ')[1])
if ($mbxVersion -lt [version]'1.7.0') {
    Write-Error "mbx 1.7.0 or newer is required; found $mbxVersionText"
    exit 1
}

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
mbx build -p omega
$ok = $LASTEXITCODE -eq 0
Pop-Location
if (-not $ok) { exit 1 }
Set-Location $root
& (Join-Path $root "target\debug\omega.exe") refresh-samples samples
exit $LASTEXITCODE
