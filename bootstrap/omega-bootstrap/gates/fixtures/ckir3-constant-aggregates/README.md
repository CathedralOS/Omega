# CKIR3 constant-aggregate source fixtures

These sources specify the source family selected by the checkpoint-000001
constant-aggregate tranche. They are fixtures, not a second language contract;
the authoritative requirements remain in `TASKS_BOOTSTRAP.md` and the
versioned lowering/checked-IR contracts.

## Positive sources

| Source | Composition | Expected observation |
| --- | --- | --- |
| `unicode-harness.omg` | Compile as a separate source unit in the **same logical module** as the exact `source/compiler/omega/psi/generated/unicode_tables.omg`. | status 0 from compilation and executable result 70 |
| `renamed-reordered-nested.omg` | Standalone source unit. | status 0 from compilation and executable result 70 |
| `guardless-transition.omg` | Standalone focused control. | status 0, one canonical CKIR `Jump`, and executable result 70 |
| `cyclic-range-custody.omg` | Standalone renamed and declaration-reordered control. | status 0 from compilation and executable result 70 |
| `arm-local-edge-argument.omg` | Standalone true-arm custody control. | status 0 from compilation and executable result 70 |
| `stale-parameter-custody.omg` | Standalone target-parameter identity control. | status 0 from compilation and executable result 70 |

`unicode-harness.omg` deliberately adds an attached machine to the private
`UnicodeTables` declaration instead of copying or editing the generated source.
It checks both fields of the first and last element of each generated array,
then checks present and absent values at the start and end of both lookup
families. The generated unit itself supplies the large recursively constant
record/array literals, aggregate assignments, nested indexing, loops, and
scalar `<=` behavior.

`renamed-reordered-nested.omg` is the generality control. Its names and array
sizes are unrelated to Unicode, record-literal fields appear in an order other
than declaration order, arrays nest through two record layers, and a completed
copy aggregate is copied before the source is mutated. The copied nested value
must remain 70. Its result is deliberately `u32`, matching the copied field's
exact type: CKIR3 adds neither numeric conversion nor aggregate-flow narrowing,
and this fixture does not make either unrelated capability part of the tranche.

`guardless-transition.omg` isolates the ordinary one-wildcard-arm source form.
It lowers to the inherited CKIR `Jump` with no synthetic Boolean value or
operation. The producer gate derives a same-result authored-`true` transition
from this fixture and requires an ordinary CKIR `Branch` with one authored
Boolean constant instead; the two publications must be byte-distinct.

`arm-local-edge-argument.omg` makes a dynamic indexed load legal only in the
true arm of `<`, including while that arm's target arguments are checked. The
gate derives a negative variant that moves the same indexed load into the
fallback arm, where no narrowing fact applies. `stale-parameter-custody.omg`
uses distinct names and identities for a predecessor parameter and its target
parameter. The proven interval must map by target ordinal rather than retaining
the stale predecessor identity.

`cyclic-range-custody.omg` isolates the Unicode lookup loop's other control
obligations without using Unicode names or dimensions: its entry is a
guardless jump; `<` narrows a scalar before that scalar enters a target state;
the target uses it for dynamic indexing; a later state forwards it; `<=` is
exercised; and a second `<` proves `index + 1` safe on the cyclic back edge. The
owner declaration deliberately precedes the element declaration. The final
element carries result 70.

## Semantic-negative sources

Every source below is expected to reject with status 251 and publish no
lowering, checked-IR, or artifact bytes.

| Source | Isolated reason |
| --- | --- |
| `negative-wrong-field.omg` | a record literal supplies an unknown field and omits a required field |
| `negative-wrong-type.omg` | a record-literal member has the wrong scalar type |
| `negative-array-arity.omg` | a fixed-array literal has the wrong element count |
| `negative-nonconstant-member.omg` | an aggregate member depends on runtime receiver state |
| `negative-noncopy-aggregate.omg` | an affine aggregate is explicitly moved into mutable receiver storage rather than copied |
| `negative-less-equal-type.omg` | scalar `<=` is applied to mismatched operand types |
| `negative-dynamic-index-no-fact.omg` | a full-interval `u32` indexes a two-element array without a narrowing fact |
| `negative-missing-field.omg` | a named record literal omits one required field |
| `negative-duplicate-field.omg` | a named record literal supplies one field twice |
| `negative-unknown-extra-field.omg` | an otherwise complete named record literal supplies one unknown extra field |
| `negative-nominal-mismatch.omg` | structurally identical but nominally distinct record types are assigned |
| `negative-out-of-range-u8.omg` | a scalar constant does not fit its destination `u8` field |
| `negative-noncopy-constant-root.omg` | a direct aggregate constant root lacks the required `[copy]` capability |
| `negative-shared-place-mutation.omg` | an aggregate constant assignment mutates through a shared `&self` receiver |
| `negative-recursive-layout.omg` | a by-value record recursively contains itself and has no finite layout |
| `negative-less-equal-bool.omg` | `<=` is applied to two nonnumeric Boolean operands |
| `negative-less-equal-structural.omg` | `<=` is applied to two aggregate operands |
| `negative-less-equal-mixed-carrier.omg` | `<=` mixes numeric `u8` and `u32` carriers |
| `negative-less-equal-missing-rhs.omg` | `<=` has no right operand |
| `negative-less-equal-chain.omg` | a second `<=` follows a completed comparison without an admitted Boolean-comparison form |
| `negative-missing-cycle-predecessor.omg` | joining entry index 0 with cyclic predecessor index 2 makes a two-element array access unsafe |

The gate also derives a declaration-reordered form of
`negative-missing-cycle-predecessor.omg`; both orders must reject. This pairs
missing-predecessor custody with fixed-point declaration-order independence
without duplicating the source fixture.

Named record literals have no positional arity form in the admitted source
grammar. Their arity failures are therefore covered honestly by the isolated
missing, duplicate, and unknown/extra-field cases rather than by a fabricated
positional-record fixture. The malformed and carrier-incompatible `<=` cases,
invalid layouts, out-of-range scalar, shared mutation, nominal mismatch, and
fact-free dynamic index are invalid full Omega. The runtime-member and affine
aggregate programs are intentionally ordinary full-Omega candidates that lie
outside this bounded constant/copy tranche; their status-251 expectation
applies to the bootstrap profile, not to the full product compiler.

## Source resource negative

| Source | Expected status | Isolated reason |
| --- | --- | --- |
| `negative-oversized-layout.omg` | 252 | a finite `[u32; 32769]` owner layout exceeds the producer's admitted 131072-byte layout ceiling |

The oversized layout is a resource exhaustion, not a semantic error, so it
must reject with status 252 and publish no checked-IR bytes. The recursive
layout above remains status 251 because no finite layout exists at all.

## Composition and syntax note

These fixtures follow the repository's current ordinary-Omega spelling:
attached machines may be supplied by another source file in the same logical
module, record literals are field-named and field-order independent, and Unit
machine calls may precede a state transition. The exact generated source has no
authored `module` declaration, so the gate must place both it and
`unicode-harness.omg` in one resolver-owned logical module. If the future CKIR3
carrier cannot attach a machine across same-module source units, that is a
resolver/lowering implementation gap; it must not be worked around by merging
or rewriting the generated source.
