Set-Location P:\
mbx nextest run -p terminal-semantics -E "test(reference_origins::memberships::tests) | test(descriptor_results::payload)" --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx build --locked -p omega --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx test --locked -p compiler --test corpus_runner --no-run --message-format=json --target-dir build/proof-profile/compiler 2> build/proof-profile/same-binary-graph/corpus-build.log | Out-File -Encoding utf8 build/proof-profile/same-binary-graph/corpus-build.jsonl
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
python build/proof-profile/same_binary_quality.py
exit $LASTEXITCODE
