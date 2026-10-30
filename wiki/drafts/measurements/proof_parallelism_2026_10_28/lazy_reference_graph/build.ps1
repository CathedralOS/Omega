mbx nextest run -p terminal-semantics -E "test(reference_origins::memberships::tests) | test(descriptor_results::payload)" --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx build --locked -p omega --target-dir build/proof-profile/compiler
exit $LASTEXITCODE
