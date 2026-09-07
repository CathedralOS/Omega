# Typed trees to checked trees

This stage checks typed programs and retains checked facts. Start at
[lib.rs](src/lib.rs). Public borrow requirements are in
[loan resources and compatibility](../../../../wiki/spec/terminal-psi/loans.md).

## Borrow evidence production

[resources.rs](src/checks/borrows/resources.rs) owns checked resource lifecycle
and restored-use planning. Resource and compatibility arenas remain separate.
Automatic non-interference retains zero-premise structural certificates with
formation coordinates, state-owned loan identities, frozen places, normalized
conclusions, and ordered selector snapshots. Replay normalizes the original
typed expression and reconstructs spatial relations and access compatibility.
This precursor does not implement general proof-premise admission or portable
compatibility evidence.

Direct reference-local reborrows require one exact prior parent. Resource rows
retain typed parent handles, activation, weakening, formation availability,
lexical end disposition, and separately checked containment. Rebuild/remap the
complete graph transactionally in loan order. Aggregate/helper transfers,
ambiguous or reassigned aliases, and write-only local loans cannot gain inferred
resource authority from these cases.

## Entry-contract readers

Entry requirements and crash routes retain incoming operands independently of
executable mutable storage. Strict readers rejoin exact live machine/entry-state
owners, symbols, parameter order, builtin type/operator meaning, and field paths.
Same-spelled locals or fields from another owner cannot supply a missing identity.
Unread structural operands and receivers still occupy authored positions;
dense scalar positions count only preceding primitive parameters.

Boolean entry readers admit direct owned formals and plain field paths through
eligible owned/shared/mutable roots. Explicit receiver paths rejoin the exact
machine-owned `Self` alias and nominal field identity. Fixed-integer comparisons
share literal landing, same-carrier construction, and numbered-field handling
with the integer-contract owner; write-only fields supply no readable hypothesis.
Comparison totality on policy-qualified inputs does not admit arithmetic, casts,
calls, or current body values as new entry facts. The Boolean-only result fallback
has a different namespace and must not be widened by sharing this construction.

Boolean polarity, compound equality, and common-consequence extraction use bounded
logical conversion (4,096 work units, depth 64), not eager runtime operations.
Conjunction combines facts; every disjunct must establish a retained common fact.
Keep original published predicate identity and exact entry ordinals. Exhaustion
does not restart the budget for another route. Selected user operators do not
inherit builtin meaning from spelling. Arithmetic-subtree, path, and case checks
remain separate from the logical expansion budget.
Bounds-guard operand recovery likewise checks the actual collection receiver,
length carrier, and builtin computation. A field spelled `len` keeps its declared
type; anonymous or unresolved operands do not inherit a sibling's type. Recovering
a type or finding a comparison declaration supplies no bounds proof.

The focused source-to-artifact regression is
[entry_requirement_crash_coverage.rs](../checked-trees-to-lowered-psi/tests/entry_requirement_crash_coverage.rs).
It verifies retained requirements and unchanged call routes, not execution of
arbitrary host records as entry proofs. Generic/lifetime attachments, broader
entry arithmetic, and mutable value-origin transport remain distinct work.

## Scalar convergence classification

[shared_convergence.rs](src/flow/terminal_unit/shared_convergence.rs) recognizes
sufficient source shapes for a shared Boolean cleanup tail. It is a producer
classifier, not proof authority or the definition of legal integer arithmetic.
The classifier families are organized by their actual expression structure:

| Owner | Recognized structure |
| --- | --- |
| `affine` | Ordered landed-literal offset/coefficient chains and bounded same-/distinct-root arithmetic joins. |
| `cast_chains` | Partial casts, strict widenings, and ordered conversion words between computed families. |
| `products` | Literal-factor multiply chains and bounded literal/runtime-divisor chains. |
| `shifts` | Ordered exact shifts and bounded cross-family/conversion compositions. |

These paths retain exact runtime parameter roots, carrier types, literal siblings,
source order, and independent operation identities. Recognizing a quadratic,
conversion, or cancellation shape does not establish that proof production or
native composition supports it. In particular, historical sufficient bounds
must not replace the operation-local questions in
[integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md).

Each arithmetic prefix and partial cast needs its own proof. A final zero factor,
cancellation, or representable conversion result cannot excuse an earlier unsafe
operation. Missing/reordered/cyclic definitions, changed roots or carriers, invalid
literal landings, and checked analysis overflow decline the candidate. A failed
bounded analysis is not a proof of mathematical falsehood.

Keep source-family limits here rather than adding a language rule per composition.
Extending a classifier alone does not extend artifact admission or erase a
downstream unsupported-shape rejection.

## Bounded Terminal correspondence

[restored-call production](../checked-trees-to-lowered-psi/src/reborrow_restored_call_use.rs)
admits a direct mutable parent with an exclusive child or one complete shared
cohort of one to three children. Final child use ends immediately before one
receiver-free whole-parent mutating call. Multi-member final observation uses
one exact shared-parameter call; earlier completed non-overlapping exclusive
siblings do not invalidate the event. Replay checks the complete cohort, both
resources, lifecycle/containment, captured places, restored access, callee shape,
and exact source call. Four-member/sequential shared cohorts, multihop or
projected restored use, direct assignment, and partial/nonmutating uses remain
outside this producer.

[root handoff](../checked-trees-to-lowered-psi/src/reborrow_root_handoff.rs)
separately admits a finite nonempty exclusive chain rooted in a direct mutable
loan and ending at state exit. Reverse and rejoin its exact retired-parent path;
reject branches and shared edges. This is direct-root custody, not a cleanup,
transfer, or linear-discharge operation. Broader Terminal evidence must be
implemented explicitly rather than inferred from a successful checked replay.
