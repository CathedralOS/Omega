# TPR6 finite selected progress subjects — Linux x86_64 evidence

Compiler advancement: `47168d09ee17e238aa0fe291d19c2a8ba60d8509`, published
through the normal queue and verified by fetch, ancestry and remote main.
The initial fresh-main baseline was
`d6cf554fd1cbf20551983d29bae26f5cf3b81664`; every positive place failed
published termination coverage. The returned-array signature was given its
specification-required explicit lifetime relation; the corrected frozen source
also fails on that baseline. No fact, operation or runtime oracle was changed.

The [nine-place row](../../../../samples/cli/facts/progress_selector/README.md)
preserves all exact `Token::Fair` subjects selected from two reference slots,
including stored helper results and state arrivals. Programs assert identities
37 and 43 and exactly two consuming calls, returning zero. Each twin removes
only the right subject's premise. Four further controls cover substituted slots,
replaced carriers, unknown alternatives and stale live contents. Owned-value and
reference-binding snapshots have positive/negative regressions too.

Actual hosted execution is Linux x86_64 only. All nine places have native exit 0
and Terminal exit 0; all 13 row rejection controls reject for their authored
reason. Four additional focused native/Terminal programs pass. The wider focused
selection has 14 accepted checks and two byte-identical baseline refusals:
`local_fixed_array_shared_state_loan_exit` needs an accepted Linux entry binding;
`owned_reference_arrival_projection_exit` lacks a typed owned-local frontier.
Those are not counted as successful executions or repaired by this change.
All four hosted entries are declared. The corpus check leg also checks the
fixture-selected macOS ARM64 target; no macOS execution is claimed.

The final immutable compiler is `integration10-omega`, SHA-256
`1ced76fe6d8ff06694671af8bcc4ac4e5a616038a5fa9946869785065995edea`,
built at `710a5764ff6626652a67206f4c1ccdbb2f643fa3`. Final execution observations
belong to `43f79ad9ec866193876b46cf2452dd07ba82603c`; qualification explicitly
retains them after unchanged compiler/consumer/oracle identities at the landed
commit. They are not relabeled as fresh executions at a later SHA. The archive
includes actual native images and `program.oterm` products compressed by digest.

Validation commands (saved Omega cloud, normal custody checks):

```sh
mbx build -p omega --target-dir build/gate-cli
cargo clippy -p typed-trees-to-checked-trees --target-dir build/gate-cli
mbx check -p typed-trees-to-checked-trees -p omega --target-dir build/gate-cli
cargo fmt -p typed-trees-to-checked-trees -- --check
python3 tools/qualification.py run-pending --state build/tpr6/qualification.json --revision 49 --base 840c4f8cf03f9b69a72c222f20e1525be8b8344e
python3 tools/corpus_gate.py --record --filter progress_selector
python3 tools/corpus_gate.py --record --native --filter progress_selector
python3 tools/corpus_gate.py --record --both --filter progress_selector
```

Cargo uses the pinned nightly toolchain with optimization 1 and the private
`build/gate-cli` directory. Compiler execution uses the supported approved host
route because sandbox UID mapping obscures root ownership; no custody or
permission rule was weakened. Qualification and landing use the same
`CARGO_TARGET_DIR=/workspace/omega-tpr6/build/gate-cli` environment.

The scoped pre-landing corpus comparison measured 30 fixtures against
`b1cadffa8d7b923b35dbcd36642484d2c2bdd983`: zero losses and 26 new outcomes.
Post-publication records add exactly 26 check rows (11 checked, 15 rejected),
11 native exit-zero rows, and 11 native/Terminal exit-zero agreement rows.
The content guard preserves every unrelated preexisting outcome, with no removed
rows. Goldens were recorded only after the compiler commit was published.

`manifest.json` identifies raw records and their uncompressed SHA-256 digests;
`runtime-artifacts.json` identifies retained program products. Qualification
receipts preserve every actual run, input closure, build profile and stream.
Owner review is explicitly non-independent. A separate source/evidence review
packet was uploaded to Library: summary
`libfile_7b00640b138c819186cc8889844f40c9`, archive
`libfile_6eabb3e23e388191be58ee8bdedcd6ff`. That earlier packet clearly separates
its built source candidate from its qualified evidence candidate.

This closes one finite-selector progress-transport row, not all of TPR6.
Generic/dispatched origins, additional reference boundaries, growing projections,
other aggregate/recursive result cases and distinct state-owned alternative
backing homes retain their task acceptance. R5 implementation is unchanged by
this lane; concurrent compiler changes were integrated and requalified.
