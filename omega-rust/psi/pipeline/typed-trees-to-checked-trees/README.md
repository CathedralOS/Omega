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
