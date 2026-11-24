# Triage packet: unit_027 (area 12, optimization unit and its whole-unit checker)

Status: in progress. One receipt so far.

This packet reviews the correctness observations from
[record unit_027](../../records/unit_027.md) and the matching lines in
[digest 12](../../digests/12-native-representations.md). The baseline is
`main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`; the record's line numbers
still hold there. Lines 3601-18242 of `unit_validation.rs` belong to the gap
packets `unit_027_gap_012`, `_013` and `_014`, not to this packet.

`UV` is `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs`.
`OU/` is `.../abstract-operations/optimization-unit/src/`. `S03/` is
`source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations/src/abstract_optimization/`.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Record §4 "Fail-open risks" 5 and "Five things" 4; digest 12 line 184: `require_accepted_goal_fact` checks only that a fact exists and the goal canonicalizes; only the `WholeRepresentation` branch compares the stored proposition | `UV:1906-1943`, `UV:2561-2579`; producer `S03/verified_unit.rs:192-283`; fact type `OU/proof.rs:270-299` | An accepted fact names the Terminal Psi obligation `(machine, owner, obligation)` that the lowered-form checker accepted, with its evidence seal. Its `proposition` bytes hold a reconstructed goal only for a `WholeRepresentation` element view. | Trace. The only producer, `source_checked_obligation_facts`, stores `Vec::new()` as the proposition for every obligation except a `WholeRepresentation` count (`S03/verified_unit.rs:209-268`). The validator compares bytes in exactly that case (`UV:1931-1943`). `grep -rn '\.proposition\b'` over `psi-to-native` finds no other reader of `AcceptedObligationFact::proposition` outside identity hashing (`OU/identity.rs:3052-3053`). A "weaker" stored proposition in the other branches is never read, so nothing can rely on it. | Intended |
