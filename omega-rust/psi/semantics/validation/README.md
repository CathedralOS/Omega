# Semantic validation

Start at [lib.rs](src/lib.rs). Source automation must preserve the
[mathematical proof contract](../../../../wiki/spec/proofs/contracts.md) and
feed the separately reconstructed [Terminal questions](../../../../wiki/spec/terminal-psi/verification.md).

## Structural algebra automation

[structural_judgment.rs](src/contract_entailment/structural_judgment.rs) retains
operation licenses and paired add/multiply semiring licenses. The paired form
requires both operations' associativity/commutativity and a conformed
distributivity law. Natural-coefficient polynomial expansion is bounded;
failure to normalize or different normal forms are not refutations.

Zero/one constructor bridging uses checked unfolding and citation, not an
unlicensed identity rewrite. Broader polynomial/index normalization, additional
Int/Rat structures, and additive/multiplicative group licenses need their own
checked contracts. Unit dimensions need additive group reasoning; positive
rational scales need multiplicative group reasoning, not integer linear
arithmetic alone.

The specialized entailment engine does not establish that arbitrary mathematical
function/predicate binders, nonconstructive values, or full foundation semantics
are implemented. Preserve useful derivations while replacing obsolete source
and evidence machinery under `PROOF-CONTRACT-MIGRATION` on the
[execution board](../../../../TASKS.md).
