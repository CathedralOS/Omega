# Proof admission

Contracts: [fact validity](../../../../../../../wiki/spec/terminal-psi/fact_validity.md),
[integer certificates](../../../../../../../wiki/spec/terminal-psi/integer_certificates.md),
and [mathematical proof values](../../../../../../../wiki/spec/terminal-psi/mathematical_values.md).

[bounded.rs](bounded.rs) exposes kernel judgments, proof checking, recursion,
normalization and witness checks a producer can call. [proof.rs](proof.rs)
admits certificates, walking them with an explicit work list rather than the
host call stack.

Admission re-decides each judgment from the supplied mathematical evidence
and runs no proof search of its own. These checks serve source proof obligations; the
Terminal artifact carries no certificate of the compiler's translation.

The checker and the interpreter evaluate closed signed mathematical integers
with the same [bounded integer evaluator](../../closed-predicates/integer.rs).
It preserves arbitrary precision and returns an unknown relation for open values
and undefined negative shifts. Resource exhaustion reports an incomplete
evaluation. The [integer proof rules](integer_rules.rs) also check relationships
between open terms. A false closed relation can establish
`relation => Falsehood`. Contradiction search retains that certificate and
the exact value equalities that closed its operands.

A raw `Identity<f32>` or `Identity<f64>` of two float literals denotes
equality of their exact bits, in admission and in the
[closed guard](../../closed-predicates/guard.rs) alike: one NaN encoding is
identical to itself, while other NaN payloads or signs and the two zeros are
not ([float equality and source identity](../../../../../../../wiki/spec/terminal-psi/mathematical_values.md#float-equality-and-source-identity)).
IEEE comparison and `FloatMeaning` identity keep their own rules.

One evaluator per invocation shares limits across operands and repeated
comparisons: 8,192 visited nodes, depth 128, 65,536 result bits,
262,144 cumulative allocation-limb units, and 1,048,576 estimated limb-work
units. These are internal limits, not integer bounds of the language. Shape
is checked iteratively before recursive arithmetic (and before primitive
mathematical propositions are formed). Pre-allocation estimates
cover 64-bit limbs, schoolbook multiplication, subtraction scratch and
growing shift vectors. Charging every allocation cumulatively bounds kept
intermediate values without a separate cache or storage ledger. A positive
unrepresentable shift count is refused as a resource limit, keeping its
mathematical meaning.

[The mathematical kernel](../mathematical/mathematical.rs) owns the common mathematical
core for the [selected foundation](../../../../../../../wiki/spec/proofs/foundation.md):
de Bruijn dependent terms, stratified `Type`/`Strict` sorts with closed levels,
Π/λ/application typing without cumulativity, dependent pairs, and typed
conversion whose strict collapse depends on the shared type's sort, never on
term shape. It also owns the first primitive of the selected
[native inductive profile](../../../../../../../wiki/spec/proofs/inductive_profile.md):
the `Two` type with `zero`/`one` and dependent `caseTwo` elimination computing
on each constructor, restricted to motives in relevant `Type`. It is the
`PROOF-KERNEL-CORE` model; the bounded `Proposition` kernel above is an
interface for producers, not a second representation of truth.
`accept_certificate` connects the two: it also translates every bounded
certificate it accepts into the core (`bounded_denotation.rs`), and the
kernel judges it when the translation covers its rule families. So a supplied
certificate discharging a theorem machine's `ensures` obligation is checked
at admission as the judgment `Γ ⊢ t : ⟦goal⟧`. The acceptance records
`Judged` with the judgment's measurements, or `Refused` naming the uncovered
family; a covered certificate the kernel rejects is rejected.

## Normalization is not authority

[integer_affine.rs](integer_affine.rs),
[integer_cast](integer_rules.rs),
[integer_shift](integer_rules.rs), and
[integer_forbidden_root](integer_rules.rs) each replay the exact witness
coordinates themselves. A checked normalization result is not a bound to
assume. Only its specified proof rule, checked child evidence and accepted
premises establish the conclusion.

Keep the checks of root, target, type, definition order, literal targets
and arithmetic here. Producer indexes and search limits may change without
changing the question or admitting an unchecked witness. Tests should alter
those coordinates, not only check successful certificates.
