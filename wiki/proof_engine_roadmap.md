# Proof engine frontier

This page describes the current source-level `requires` to `ensures`
automation boundary. The target proof architecture is in
[`design_briefs/proof_engine_north_star.md`](design_briefs/proof_engine_north_star.md),
and the certificate work queue is in [`../TASKS.md`](../TASKS.md).

## Live automation

`source/omega-rust/psi/semantics/psi-validation/src/contract_entailment.rs` checks the
supported contract fragment rather than accepting unsupported conclusions as
proof. Its current vocabulary includes:

- canonical integer polynomials with constant folding, congruence,
  commutativity, and distributivity;
- directed substitutions from `requires` equalities;
- difference-bound closure for ordering, antisymmetry, transitivity, and
  vacuity;
- correlated interval reasoning for ranges, sums, powers, and Euclidean
  remainder bounds; and
- accumulator-style self induction. Each recursive hypothesis is gated by a
  proved strict decrease of the declared natural ranking measure at that call
  edge.

The L0-L7 corpus covers constant arithmetic, order reasoning, range sums,
polynomial identities, square bounds, remainder bounds, and ranked inductive
theorems. True programs live under `tests/omega/pass/proofs`; false twins under
`tests/omega/fail/proofs` must reject. The sample and lattice copies under
`samples/cli/proofs/math_proofs` and `tests/lattice/corpus/math_proofs` are
readable demonstrations, not a second specification.

## Trust boundary

This source entailment engine is still trusted automation. It does not yet emit
the kernel-checkable terminal-Psi certificate required by the proof-carrying
artifact model. That bridge must:

- record one well-founded ranking relation per recursive strongly connected
  component and a separate strict-decrease proof for every internal edge;
- cite exact selected conformances and laws for normalization;
- retain the transitive trust closure of admitted premises; and
- derive the human synopsis deterministically from the checked certificate and
  its source-attribution metadata.

The terminal artifact must determine its obligation set. A proof bundle may
discharge those obligations but may not define, omit, or replace them.

## Unsupported frontier

- General quantified propositions and quantified sequence facts are not in the
  checked source fragment.
- Proof views and the broader typed `Nat`, `Int`, `Rat`, sequence/Cauchy, and
  `Real` corpus are incomplete.
- Mutual or general recursive proof bodies need the component certificate rule;
  the current automation recognizes the narrower accumulator-style self shape.
- Contracts outside the readable fragment do not become theorems merely because
  the legacy automation stands down. They require the future typed calculus or
  explicit admitted evidence, and every admission must remain visible.

The open implementation and acceptance conditions are maintained only in the
P3 `PROOF-CERTIFICATION-BRIDGE`, PCC verifier closure, and proposition tasks in
`TASKS.md`; do not append landing logs here.
