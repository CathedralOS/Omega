# Triage packet: unit_026 (area 12, `optimization-unit-semantics`)

Status: in progress.

This packet reviews the open correctness observations from
[record unit_026](../../records/unit_026.md) and the matching lines in
[digest 12](../../digests/12-native-representations.md). The baseline is
`main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. At that revision the six
unit files keep the line numbers the record cites. No supplemental gap range is
assigned to unit_026. The second-pass ranges in this crate cover
`unit_validation.rs`, which belongs to unit_027.

`S/` is
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/`.
"Fail-closed" means the code path can only refuse more programs or keep more
state alive. It cannot accept a program that the full check would refuse.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| A | Record §`current_ownership.rs` "Fail-open risks", bullet 2; digest 12 line 202: `discard_subtree` returns `Ok` when the place has no contract (`:6393-6395`) | `S/current_ownership.rs:6385-6445` | Releasing a reference subtree removes exactly the live references carried by `source` under `prefix`, after the child-before-parent check. | Trace. The early return leaves `live` unchanged, so no reference is released. `validate_normal_return` (`:7998-8030`) refuses unless the live set equals the expected borrowed set exactly. A carrier without a contract would therefore keep its references live, and the return check would refuse the program. The ownership side of each discard is checked by the callers before the call (`:3975-3991`, `:4155-4165`). | Intended (fail-closed) |
| B | Record §`current_ownership.rs` "Fail-open risks", bullet 3; digest 12 line 202: `ValueUse::moving` and `observing` return `Ok` for non-owned roots or paths that leave inline storage (`:10429-10436`, `:10484`, `:10515-10528`) | `S/current_ownership.rs:10411-10534`, replay loop `:8543-8548`, `:8956-9000`, `:9202-9240` | `moving` and `observing` add the claim-frontier check for inline owned storage. They are not the only liveness check. | Trace. `observing` refuses a moved or partly moved owned root before any inline-storage return (`:10487-10509`). For every owned argument, the replay loop checks liveness separately: a whole move must remove the place from `owned_places` (`:8956-8962`), and a projected move needs a live owner and a non-overlapping path (`:8964-8991`). The only exception is `is_borrowed_member_move`. Borrowed roots go through reference custody and the region checks. The stated limit at `:7175-7178` covers owned extraction through a borrowed root. `store_tests` at `:11306-11308` pin `observing` on a partly moved root. | Intended |
| C | Record §`current_ownership.rs` "Fail-open risks", bullet 4; digest 12 line 202: `AddressJoinPins::new` returns empty pins when the join lists are empty (`:1960-1966`, `:2043-2053`) | `S/current_ownership.rs:1914-2090`; `is_address_join` in `structural-storage/queries/src/address_joins.rs:22-35` | Shared pins exist only for shared address joins. Exclusive pins exist only for exclusive address joins. | Trace. Shared pins come from `joins`, a subset of `address_joins` (`:1924-1939`, `:2007-2020`). Exclusive pins come from `exclusive_joins`, also a subset (`:2029-2042`). Each early return gives the same result as the full computation would for an empty set, and both keep `borrowed_regions` and `descriptor_loans`. `is_address_join` accepts every borrowed, unrestricted, non-view block parameter. Borrowed views take the descriptor-join path (`is_descriptor_join`, `:39-60`), which `DescriptorLoans` checks. | Intended |
