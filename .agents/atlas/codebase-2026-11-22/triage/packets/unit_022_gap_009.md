# Packet unit_022_gap_009: replay checks in image-emission `function_fragments.rs` 7801-13800

Packet complete. One claim was an ordinary bug, reproduced by trace and fixed here. A structural case
dispatch's cleanup calls were never checked. Every other claim is intended, already closed by another
check, or a stated refusal. One lead outside this range is listed at the end, unresolved.

Scope: area 11 only, [record unit_022_gap_009](../../records/unit_022_gap_009.md) and
[units/gap_009.tsv](../../units/gap_009.tsv) (`function_fragments.rs` lines 7801-13800). Baseline
`24ad2700685e6cc6062026c87ed5fd24ed415265`. The file was byte-identical at that baseline and at the review
parent `a0152d26`, so every `FF:` line below is a baseline line. The repair shifts later lines.
`FF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs`;
`RSV` = `.../image-emission/src/function_fragments/source/retained_source_validation.rs`.
Digest references are to [11-native-stages-11-15.md](../../digests/11-native-stages-11-15.md). No merged
row in [07-13-psi-and-native.md](../07-13-psi-and-native.md) and no open PR cited this range when review began.

## Summary

| # | Claim (original location) | Outcome |
| --- | --- | --- |
| C1 | Cleanup rosters of four terminators compare only the `InvokeOwned` count (record §2; digest §5, §6) | intended |
| C2 | `Resident` backing accepted with no allocation check (record §3a; digest §5) | intended |
| C3 | Non-`BoundaryCall` machine calls left out of the descriptor graph (record §3a; digest §6) | intended (traced closed) |
| C4 | Empty activation-residence roster accepts (record §3a) | intended (traced closed) |
| C5 | `FrameAddress`/`CopyI64` definitions accepted regardless of provenance (record §3b; digest §5) | intended (traced closed) |
| C6 | Uninvoked callable adds no stack edge (record §5; digest §5, §6) | intended (stated) |
| C7 | Empty dispatch-table slot adds no stack edge (record §5; digest §5, §6) | intended |
| C8 | Call targets other than `Callable`/`Dynamic` skipped by `stack_calls` (record §5; digest §5, §6) | intended (traced closed) |
| C9 | One-case structural dispatch does not replay the tag read (record §6; digest §5) | intended |
| C10 | `Local` dispatch source has no address replay (record §6; digest §5) | intended |
| C11 | Owned parameter accepted for a borrowed dispatch source (record §6; digest §5) | intended |
| C12 | Case-edge cleanup calls of a structural dispatch are never checked (found tracing C1 and C9) | **reproduced (trace), fixed** |
| D1-D6 | Duplicate helpers (record §2, §3a, §3b; digest §3) | duplicate code, no wrong result found |
| L1 | Closed shape lists: address-forming kinds, case successor kinds (record §3b, §6; digest §4) | intended |
| L2 | Stated backing and stack refusals (record §3a, §5; digest §8) | feature gap (stated, fails closed) |
| L3 | History comment `FF:10693` (record §3b; digest §8) | intended |

## Claims

### C1. Cleanup rosters of `InvokeNoNormal`, `ExternalCompletion`, `EliminateEmpty`, `Crash`

Record §2 fail-open 1 and "What you are unsure about" 2; digest §5 `FF:8325-8327` with `:8075-8174`, §6 first `FF` bullet.

- Code path: `control_flow::retained` (`FF:8004`) calls `cleanup_rosters_match` (`FF:8270-8293`).
- Contract: a terminator's block must hold exactly the cleanup calls of the `InvokeOwned` actions in its rosters.
- Trace: `AbstractOperation::cleanup_rosters` (`abstract-operations/src/operations.rs:800-838`) yields rosters only
  for `Jump`, `Return`, `ReturnUnit`, `ReturnStructural`, `Conditional` and `StructuralCase`. The four named
  operations have no cleanup-action field (`operations.rs:672-701`), and neither do their target terminators
  (`target-operations/src/control_flow.rs`). For them the expected count is 0, so `FF:8290` refuses any
  `CleanupCall` row in the block. For terminators with rosters the full action list is compared
  (`FF:8188`, `:8211`, `:8239`, `:8267`; `ReturnStructural` at `FF:7965-7992`). `DiscardRoot`, `RetireHome`
  and `DiscardResidual` produce no call row in target lowering either (`control_flow_calls.rs:1864-1868`).
- Outcome: **intended**; fails closed. No fix.

### C2. `Resident` backing accepted without an allocation check

Record §3a fail-open 1 and "Five things" 3; digest §5 `FF:10225-10244`.

- Code path: `BackingGeometry::proves` (`FF:10211`), `Resident` arm `FF:10225-10244`, then
  `placed_reference::retained` (`FF:11265`).
- Contract: a resident owner lends the allocation its Extent record points to
  (`operation-operands/src/access_references.rs:1-5`). [Access policies](../../../../../wiki/spec/resources/access.md)
  says `Extent::Resident<P, T>` owns one complete live `T` over the exact placement (line 307). That custody is
  checked by source semantics, not by the native frame.
- Trace: no native allocation exists to measure. The resident's bytes live wherever its base pointer points.
  The replay checks what it can see: a unique access-view plan for the carrier, the same schema and the same
  placement commitment (`FF:10226-10242`). `validate_backing` checks the same commitment, owner base and length
  fields and distinct roles (`structural-storage/queries/src/access_reference_validation.rs:38-49`).
  `placed_reference::retained` then requires the reference to be the pointer loaded from the owner's base field:
  `address_source` appends the base field (`access_references.rs:44-59`) and `geometry_address` runs with
  `resident = true` (`FF:11303-11315`). A different address fails `captured != expected` (`FF:11367`).
- Outcome: **intended**. No fix.

### C3. Calls whose original is not a `BoundaryCall` are left out of the descriptor graph

Record §3a fail-open 2; digest §6.

- Code path: `record_selected_formation_calls` (`FF:10050-10110`), lookups through `invocation` (`FF:9891-9922`).
- Trace: a skipped call has no `known_invocations` row with its owner, so `invocation` returns
  `Err("selected call has no unique reference-origin owner observation")` whenever a backing trace needs it.
- Outcome: **intended**; the record's trace holds and the path fails closed.

### C4. Empty activation-residence roster accepts

Record §3a fail-open 3.

- Code path: `activation_residence::validate` (`FF:8837-8909`).
- Trace: `actual != required` is refused first (`FF:8874-8878`). An empty call list therefore means the
  requirement roster is empty too, and there is nothing to prove.
- Outcome: **intended**; record's trace confirmed.

### C5. `FrameAddress` and `CopyI64` definitions accepted regardless of provenance

Record §3b fail-open 1; digest §5 `FF:10892-10896`.

- Code path: `definition` (`FF:10868-10911`) inside `AddressReplay::address`.
- Trace: the row must still be the unique writer of the register (`inputs.unique`, `FF:10901`), and the register
  origin must name that row (`FF:10904-10909`). A `FrameAddress` contributes its exact slot as the base, and a
  `CopyI64` is followed to its own input. The rebuilt `Address` is compared with the geometry address, so a
  copied or foreign frame address cannot stand in for a different slot.
- Outcome: **intended**; bounded as the record says.

### C6. An uninvoked callable adds no stack edge

Record §5 fail-open 1; digest §5, §6 `FF:12353`.

- Code path: `stack_calls` (`FF:12338-12354`).
- Trace: the call is skipped only when the caller is not in `reached_machines`. A reached caller with a missing
  occurrence is refused (`FF:12343-12350`). The comment "Retaining an uninvoked body adds no activation demand"
  (`FF:12352`) states the rule.
- Outcome: **intended (stated)**.

### C7. An empty dispatch-table slot adds no stack edge

Record §5 fail-open 2; digest §5, §6 `FF:12424`.

- Code path: `stack_calls`, `Dynamic` arm (`FF:12391-12440`).
- Contract: a `None` row means that implementation table has no callee for the slot. Every owner treats it that
  way. The Psi synchronous call graph skips it (`psi/semantics/src/synchronous_calls.rs:291`), and
  `public_realizations` lists only present rows (`descriptor_results/invocation.rs:419-424`). The wire
  validator skips it (`psi/artifact/wire/src/semantic_module.rs:1095-1098`), and the interpreter's reachability
  scan does too (`interpreter/instructions/execution.rs:519-525`). A call that selects an empty row is refused
  by the interpreter (`interpreter/instructions/calls.rs:3053`). An empty row pushes no callee frame, so it adds
  no stack demand.
- Outcome: **intended**. Native behavior when a call selects an empty row is outside this slice and was not checked.

### C8. Call targets other than `Callable` and `Dynamic` are skipped by `stack_calls`

Record §5 fail-open 3 and "unsure" 3; digest §5, §6 `FF:12391-12396`.

- Trace: `NativeCallTarget` is `AbstractCallTarget` (`target-vocabulary/src/calls.rs:247`) with three variants:
  `Machine`, `Callable` and `Dynamic` (`operation-operands/src/calls.rs:59-70`). Only `Machine` reaches the
  `else continue`. Stage 13's exit contract accepts a selected machine call only as an internal call with its
  fixup (`13_.../exit_contract.rs:1156-1160`). Text placement turns every such fixup into a
  `resolved_internal_machine_calls` row, or fails with `MissingInternalMachineTarget` or `SourceShapeMismatch`
  (`13_.../text_placement.rs:642-693`). `stack_calls` takes machine edges from that list (`FF:12314-12319`).
- Outcome: **intended (traced closed)**.

### C9. A one-case structural dispatch does not replay the tag read

Record §6 fail-open 1 and "unsure" 4; digest §5 `FF:12528`.

- Code path: `structural_case::retained` (`FF:12528-12541`).
- Trace: instruction selection emits no tag load for a one-case dispatch. When `tested` is empty it emits
  only an unconditional `Jump` to the single case (`05_.../selection_scalar_graph.rs:6639-6654`). The tag
  `Load32` comes after that return (`:6655-6690`). Runtime index selectors are still replayed for every
  dispatch through `dispatch_indices` (`FF:12516-12527`), and the single `Semantic` successor is matched per
  case (`FF:12558-12587`).
- Outcome: **intended**: there is no tag read to replay.

### C10. A `Local` dispatch source has no physical address replay

Record §6 fail-open 2; digest §5 `FF:12644-12648`.

- Trace: selection produces `Local` only for a frame slot with no runtime indices and no pointer slots.
  Otherwise it rebuilds the source as `Borrowed` with a computed pointer (`selection_scalar_graph.rs:6600-6637`).
  The replay mirrors that: `Local` refuses `Referent` and runtime-index paths and checks the slot's owner
  (`FF:12644-12648`, `source_slot_matches` `FF:12702-12771`). The tag read from a `Local` slot is still
  address-replayed when there is more than one case. `dispatch_read_matches` calls `case_address_matches`,
  which compares against the allocation base (`FF:11592-11631`, `:11702-11744`). `dispatch_indices` checks
  `byte_offset` and `region_byte_size` against geometry (`FF:11655-11661`).
- Outcome: **intended**: a frame slot has no pointer computation to replay.

### C11. An owned parameter is accepted for a `Borrowed` dispatch source

Record §6 fail-open 3; digest §5 `FF:12746-12748`.

- Trace: selection passes an owned parameter that has no local home by pointer and records it as `Borrowed`
  (`selection_scalar_graph.rs:6530-6559`). The replay accepts exactly that pair, and
  `addressed_case_parameter_keeps_its_declared_owner_and_read_access` (`FF:12794`) pins it.
- Outcome: **intended**.

### C12. Case-edge cleanup calls of a structural dispatch were never checked (fixed here)

Found while settling C1 and C9; not listed in the record or digest. Original section: record §6, `structural_case`.

- Problem: the retained-source audit accepted a structural case block whose cleanup calls were dropped,
  duplicated, re-targeted or sent to the wrong case edge. It checks every other terminator's cleanup calls.
- Code path: `RSV:834` sends `AbstractOperation::StructuralCase` only to `structural_case::retained`
  (`FF:12454-12627`). That function compares each selected successor's `cleanup_actions` metadata
  (`FF:12609`) but reads no `TargetUnitOperation::CleanupCall` row. `correspondence::validate_block`
  (`FF:7135-7177`) checks only that trailing cleanup rows carry no authored source. `control_flow::retained`,
  which runs `cleanup_rosters_match`, never sees a `StructuralCase` terminator: its candidate filter has no
  `StructuralCase` arm (`FF:8010-8061`). The only other reader, `incoming_calls::IncomingCalls::new`
  (`FF:9688-9740`), records a mismatch as an `Err` input, which surfaces only if a backing trace passes
  through that call.
- Contract: target lowering emits a `CleanupCall` for every `InvokeOwned` action in every roster of a
  terminator, case edges included, into the dispatching block (`04_.../target_lowering/control_flow.rs:3285`,
  `control_flow_calls.rs:1854-1932`). Selection carries those calls on each case successor
  (`selection_scalar_graph.rs:6889-6901`). `AbstractOperation::cleanup_rosters` includes the case rosters
  (`operations.rs:830-836`).
- Reproducer: by trace (above). `structural_case::retained` has no positive fixture, and building one needs
  a full selected function, address replay and optimization unit. The behavior is pinned at the shared
  helper with case-shaped rosters instead.
- Fix (`psi-to-native` image-emission, two commits):
  1. Code: `cleanup_rosters_match` now takes any roster iterator in one pass instead of an
     `AbstractOperation`. `control_flow::retained` passes `source.cleanup_rosters()`, and
     `structural_case::retained` passes its cases' `(psi_edge, cleanup_actions)` against the dispatching
     target block's operations. One mechanism now covers every terminator with cleanup rosters.
  2. Fixture: `control_flow::cleanup_tests::case_dispatch_block_retains_every_case_cleanup_call`. Two case
     edges carry rosters `[DiscardRoot, InvokeOwned]` and `[InvokeOwned]`. The matching calls are accepted,
     and an empty roster set refuses them. Five mutations are refused: a dropped call, a duplicated call, a
     call moved to the other edge, a changed callee, and a changed subject place.
- Before: on the parent, `structural_case::retained` read no `CleanupCall` row (code above), so none of
  those mutations in a dispatching block could be refused. After: see validation in the PR.
- Outcome: **reproduced (trace), fixed**.

### D1-D6. Duplicate helpers

Record §2 "Same job elsewhere", §3a, §3b; digest §3 `FF` bullet.

- D1, cleanup-call matchers that trust different fields (`FF:8307`, `:9868`). `cleanup_source_matches`
  identifies which source cleanup site a target call belongs to, for backing traces. `edge_cleanup_calls_match`
  checks the call's geometry. Both run on the same rows, and C12 now applies the geometry check to case
  blocks too. No wrong result.
- D2, two `precedes` (`FF:10563` linear scan, `:10730` sorted table); D3, a second defining-row
  classification in `source_value` (`FF:10803` vs `:10676-10703`). Both pairs count a defining instruction
  once and end in the same `precedes_blocks`. No disagreement was demonstrated.
- D4, twin graph and `BackingGeometry` constructors (`FF:8821`/`:10169`, `:8911`/`:10191`). They differ only
  in the catalog function and slot kind, which is the intended specialization.
- D5, block position computed three times (`FF:9217`, `:9252`, `:10036-10044`). D6,
  `StructuralReplay::from_catalog` rebuilt for every roster check (`FF:8276`, `:8301`); a cost, not a
  correctness defect.
- Outcome: **duplicate code, no wrong result found**. No fix in this packet.

### L1. Closed shape lists

Record §3b "Cases by shape", §6; digest §4 (`FF:11219-11261`, `:12559-12566`).

- The address-forming kinds end in `_ => None` (`FF:11260`), so a new kind is refused, not accepted.
  Selection builds case successors only from a `ConditionalBranch` chain and one `Jump`
  (`selection_scalar_graph.rs:6639-6700`), which is the list the replay searches.
- Outcome: **intended**.

### L2. Stated refusals

Record §3a, §5 "Stated limits"; digest §8 (`FF:9090`, `:9095`, `:10223`, `:10410`, `:12363-12365`).

- Generic and opaque backing, borrowed extents, entry or dynamic backing inputs, and callable stack demand
  with unresolved code are all refused with a named error. That rejects a program the audit cannot judge; it
  never accepts one.
- Outcome: **feature gap (stated, fails closed)**.

### L3. History comment at `FF:10693`

- The comment explains why `SelectedAddressInputs::new` deduplicates by instruction ordinal: to keep the
  earlier predicate's count-once rule. The code does that (`FF:10693-10700`).
- Outcome: **intended**.

## Unresolved, outside this range

- L4 (record "What you are unsure about" 1). `RSV:806-808` accepts an `AbstractOperation::Return` with an empty
  `cleanup_actions` without calling `control_flow::retained`, so its returned value and terminator are not
  replayed in this function. `ReturnUnit` does the same (`RSV:809-812`). `RSV` belongs to another unit, and I
  did not trace whether another check replays cleanup-free scalar returns. **Still open**; next step: find
  whether any audit compares a `ReturnScalar` terminator's `source_value` with the abstract `value`.

## Validation

Repair commits: `d5c1df6d` (code), `9c3ca20d` (owning test), on parent `a0152d26`.

- `mbx clippy -p image-emission --all-targets -- -D warnings`: clean.
- `mbx test -p image-emission --lib -- control_flow::cleanup_tests structural_case`: 7 passed, including
  `case_dispatch_block_retains_every_case_cleanup_call`.
- `mbx test -p image-emission`: 122 lib tests and 5 integration tests passed.
- Build workaround, not committed: on the parent, `optimization-unit` and `optimization-unit-semantics` do not
  compile. #5393 added `Proposition::DeclarationIdentity` without arms in
  `optimization-unit/src/identity.rs:267` and `optimization-unit-semantics/src/unit_validation.rs:10668`.
  Open PR #5432 repairs the first. These runs applied #5432's `identity.rs` hunk plus a local
  `DeclarationIdentity` arm in `proposition_structural_roots` that collects `Input` subject roots and `Scalar`
  term roots. That arm is not part of this change and still needs a repair on `main`.
- Not run: a native or Psi program run. No program exercises the retained-source audit with a mutated case
  cleanup call, and the audit runs only on an explicit validator request with retained stage 02.
