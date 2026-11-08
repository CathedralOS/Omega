# Named logical negation retains mathematical identity

This PROOF-KERNEL-CORE repair lets a named mathematical predicate negate a formed
`core::Identity` without treating that statement as an executable Boolean. The
[positive](main.omg) requires a distinction between two records, double negation
of a reflexive identity, and the raw representation distinction between IEEE
negative and positive zero. Its executable record-field calculation must produce
46 or trap, with native and Psi execution both exiting zero.

The [unresolved-subject control](../../../fail/proofs/mathematical_negated_identity_unknown/main.omg)
removes any justification for distinguishing the exact record subjects. It must
fail its `ensures`; forming identity or failing to prove identity supplies no
negation. The [named known-equal Nat control](../../../fail/proofs/mathematical_negated_identity_refuted/main.omg)
uses a named negation under an established identity and must be refuted
structurally. The original [direct Nat control](../../../fail/proofs/nat_structural_disproof_refuted/main.omg)
retains direct source negation of that same established identity. Together these cover proven, refuted and unresolved operands.

The governing contracts are [explicit identity](../../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts),
[separate judgments](../../../../../wiki/spec/proofs/foundation.md#separate-judgments),
and [mathematical definitions](../../../../../wiki/spec/proofs/mathematical_bindings.md#named-mathematical-definitions).
Identity retains its exact carrier and subjects, including IEEE representation
bits. Logical negation reverses a justified judgment and preserves Unknown.
It supplies no ownership or access authority and remains erased at runtime.

## Baseline and validation scope

The frozen positive refused all three named obligations with unchanged compiler
semantics at `0bda069e4d2c828a632bcc0fbebd97e482738f5f`, whose compiler and library
sources equal fixed base `40ae51f95cce41abafbfc21dbea8f5e2ca2cb20c`.
[Baseline run 42509455804](https://github.com/CathedralOS/Omega/actions/runs/42509455804)
retains that positive, the original direct Nat control, source archive, optimized
producer profiles, and diagnostics. The unresolved-subject control rejected for
its intended obligation. The valid positive and its exit-zero oracle stay frozen.

Focused candidate validation covers this pair, the named Nat false control,
existing identity controls, integer negation and its double-negation composition,
and selected custom and IEEE comparisons. The existing
[negated integer-order row](../../../../../samples/cli/facts/negated_mathematical_order/README.md)
retains nine-place coverage of its separate integer-complement path. This repair
does not close PROOF-KERNEL-CORE's broader acceptance or claim foreign-host
execution. Raw runs remain outside git under the existing qualification workflow.
