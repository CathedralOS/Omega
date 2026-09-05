# Runtime reference index controls

Run `sh tests/epsilon/runtime-references/run.sh` from the repository root.
The selected Delta compiler compiles the complete Epsilon evaluator plus this
ordinary Delta test closure. The selected Gamma evaluator executes the receipt.
Host code only packs bytes, verifies identities, and compares exact output.

Start at [`main.delta`](main.delta). [`states.delta`](states.delta) checks first
Complete versus last Resolved selection, same-start kind/span collisions, missing
references, and invalid-extent fallback. [`transitions.delta`](transitions.delta)
checks subject identity and order, completed-pattern identity and order, and
refusal to promote unfinished pattern premises. Each control query emits the
index representation tag followed by the old linear and new indexed selections;
the expected bytes require an actual index on valid inputs as well as equal
lookup results. [`expressions.delta`](expressions.delta) covers expression facts
and cross-ledger fallback.

These are synthetic checked-fact records, not admitted Epsilon programs. They
test derivative lookup behavior that well-formed source alone cannot exercise,
including duplicate facts and invalid index premises. They do not establish
language conformance or replace the checking and execution suites.

[`runtime_references.delta.sources`](runtime_references.delta.sources) selects
and identifies every test member. [`receipt.tsv`](receipt.tsv) binds the emitted
receipt, and [`expected.hex`](expected.hex) records the independently specified
observation. Generated source and receipts stay outside the repository.
