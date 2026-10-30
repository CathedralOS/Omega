Set-Location P:\
rustfmt --check --edition 2024 --config skip_children=true omega-rust/psi/compilation/representations/terminal-psi/semantics/src/reference_origins/memberships.rs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx nextest run -p terminal-semantics -E "test(reference_origins::memberships::tests) | test(descriptor_results::payload)" --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
mbx clippy --locked -p terminal-semantics --lib --target-dir build/proof-profile/compiler
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
python build/proof-profile/qualify_graph_lazy.py
exit $LASTEXITCODE
