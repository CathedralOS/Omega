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
