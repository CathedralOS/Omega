Set-Location P:\
mbx nextest run -p artifacts -p terminal-semantics -E "package(artifacts) | test(reference_origins::memberships::tests) | test(descriptor_results::payload)" --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx build --locked -p omega --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
python build/proof-profile/freeze_core.py graph-phase-cpu
exit $LASTEXITCODE
