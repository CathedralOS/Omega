# Build the optimized development CLI, then run every applicable sample's native
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
Set-Location $root
# The gate builds the opt-level 1 CLI it measures into build\gate-cli\.
python tools/sample_native_gate.py
exit $LASTEXITCODE
