# Target Operations To Assigned Target Operations

[Pipeline](../pipeline.md) | Previous: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md) | Next: Assigned Operations To Machine Code

This stage assigns physical homes such as registers, stack slots, spills, and calling-convention locations.

## Stage Contract

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and calling-convention homes.

## Implementation Map

- `src/lib.rs` is the crate map; `assignment/mod.rs` owns entry-roster checking
  and the target-plan to assigned-plan join.
- `assignment/function/mod.rs` retains function identity around a 43-line
  exhaustive carrier-family router. Cleanup, boundary, Unit, scalar, direct
  structural parameter, and structural-call families descend into named
  leaves.
- `assignment/{placement,control,expressions}/` owns physical-location checks,
  nested control assignment, expression frames, typed expression trees, and
  independent parameter-location discovery.
- `assignment/function/unit.rs` replays attached-Unit scalar-call plans and
  sources, reduces each argument to an exact register or outgoing-stack
  destination, and assigns one ordered, non-reused scalar result home after
  the structural parameter-home prefix.
- `assignment/function/unit/operation.rs` applies the same exact plan/source
  replay to ordinary Unit-result calls. Its bounded forwarding lane binds an
  incoming native fixed-integer or Boolean caller parameter, or one exact
  preceding Boolean literal, to the scalar prefix of the callee ABI while
  keeping structural copies in the remaining suffix. Machine emission admits
  the exact same-register case or emits the literal directly and records its
  complete call custody. Object construction and installation independently
  replay the caller source, Unit callee ABI, scalar transfer, structural copy,
  stack evidence, and target call bytes.
- `assignment/function/unit/structural_scalar.rs` independently reconstructs
  the bounded projected integer-field store and structural scalar call,
  including carrier layout, source definition, field offset, projected copy,
  and exact call ABI.
- `assignment/function/unit/operation.rs` owns the bounded x86 nearest-FMA XMM
  homes and replays each raw-bit constant source before producing an assigned
  FMA operation. The selected plan and admitted provider remain semantic
  custody; this stage does not encode machine bytes or establish MXCSR.
- `assignment/function/unit/installed_provider.rs` owns the ordinary-path
  physical assignment for the exact one-`i32`, Unit-returning selected-provider
  call. It rejoins the caller's scalar parameter placement, canonical callee
  plan, provider, and empty structural/claim rosters before binding the first
  native argument register. Zero-scalar installed providers remain routed to
  the distinct optimized structural continuation.
- `assignment/function/dynamic_parameter.rs` owns final descriptor-slot
  dispatch and transparent scalar or Unit parameter forwarding. Both
  forwarding forms independently replay their two-word call plans and require
  the incoming descriptor registers to be the exact outgoing helper registers;
  the Unit form retains no result, and neither reauthenticates the parameter as
  a local selection.
- `assignment/function/unit/dynamic.rs` also owns aggregate-stored descriptors.
  Establishment allocates one aligned runtime-ABI two-word home; the later
  indirect call must rejoin the same target-neutral custody, source projection,
  descriptor ABI, and byte offset from the unique preceding assigned store.
  Only after that join does assignment allocate the distinct scalar result
  home. Machine emission consumes this split literally: establishment writes
  the instance and private-table address into the shared home, while the later
  operation reloads the same two words for indirect dispatch and records the
  exact target bytes, relocation fields, selected slot, and scalar result.
  Object and final-image replay independently validate that evidence and bind
  the complete private table. Installation format 68 retains the two source
  operations, shared descriptor home, application/table selection, source
  place, realization, and exact establishment/call text intervals as one
  canonical stored-call row. Installation validation independently reconstructs
  that projection from the final image and rejects ordering, table, source, or
  interval substitution. When the stored result feeds the bounded immediate
  control diamond, subsequent scalar-source assignment recognizes only this
  call's exact durable result-home requirement; another dynamic-call family or
  reconstructed value cannot satisfy it.
- `omega-assigned-target-operations/src/lib.rs` owns the output representation.
- This is the bounded compatibility continuation. The selected-instruction,
  liveness, and allocation continuation is its durable replacement; neither is
  a source-shaped fallback backend.
## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Become concrete stack/runtime homes or target-addressable memory shapes. |
| Values | Receive assigned homes such as immediates, stack slots, runtime storage, runtime pointees, indexed runtime-frame locations, or scratch registers; target value summaries are preserved as assigned value metadata. |
| Facts | Diagnostic metadata only; this stage does not discharge proof obligations. |
| Loans | Prior-stage invariant only; borrow state is not rechecked here. |
| Moves | Preserve target ownership summaries while physical homes are assigned; explicit assigned transfer operation lowering is still pending. |
| Drops | Preserve target ownership summaries while physical homes are assigned; explicit assigned cleanup operation lowering is still pending. |
| Calls | Receive physical ABI placement when represented by target operation metadata. |
| Transitions | Receive concrete branch/linkage operands where possible, without changing control-flow shape. |
| Effects | Remain operation metadata attached to already-authorized operations. |
| Boundary edges | Preserve target boundary-edge summaries, including source/lowered links and policy-check records, while host-call operands receive physical ABI placement. |

## Ownership Rules

- Must not own object encoding, final bytes, semantic validation, proof discharge, or borrow checking.
- Must keep register/stack assignment policy here instead of leaking it backward into target operation construction.
- Must preserve target operation ordering unless a later allocator explicitly owns reordering.
- The bounded O0 attached-Unit scalar lane does not reuse result homes; reuse is
  a later allocator decision requiring its own liveness evidence.
- Direct Boolean control accepts only the exact preceding one-byte Boolean
  home and rejects producer, value, scalar-type, or shape substitution.
- The bounded nearest-FMA lane currently uses fixed XMM homes. Widening or
  reusing them is an allocator change and must preserve per-occurrence custody.

## Known Gaps

Current scratch register assignment is fixed and minimal. Real register allocation, spill insertion, and full stack-frame assignment should grow here or in narrow modules immediately under this stage.
Ownership summaries are preserved through assignment but not yet lowered into
assigned copy/cleanup operations.
Boundary-edge summaries and target boundary policy-check records are preserved
through assignment. The callback registrar backend replay separately binds the
opted-in target host-operation provenance to the identical assigned instruction
and operand handles before any object relocation is permitted.
Value summaries are preserved through assignment, but their storage/drop
consequences are still metadata rather than explicit assigned cleanup or move
operations.
The installed-provider scalar continuation does not yet allocate a durable
caller home. Consequently the target stage rejects repeated calls from the
caller-saved incoming register; widening that cohort requires explicit
preservation/reload custody here.
