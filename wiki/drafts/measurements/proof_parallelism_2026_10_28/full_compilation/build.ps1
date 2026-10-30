mbx nextest run -p phase-profile --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx build --locked -p omega --target-dir build/proof-profile/compiler
exit $LASTEXITCODE
