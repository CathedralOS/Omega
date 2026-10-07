# Rebuild the Rust development CLI, then run every applicable sample's native
# oracle through the repository-owned sample gate. Sample coverage does not
# belong to the shipped `omega` command surface.
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
python tools/sample_native_gate.py --cli (Join-Path $root "target\debug\omega.exe")
exit $LASTEXITCODE
