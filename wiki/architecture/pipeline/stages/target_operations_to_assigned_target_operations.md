# Target Operations To Assigned Target Operations

[Pipeline](../pipeline.md) | Previous: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md) | Next: Assigned Operations To Machine Code

This stage assigns physical homes such as registers, stack slots, spills, and calling-convention locations.

## Stage Contract

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and calling-convention homes.

## Implementation Map

- `omega-target-operations-to-assigned-target-operations/src/lib.rs` owns the
  compatibility assignment boundary.
- `structural_result.rs` and `structural_scalar.rs` own the currently supported
  physical families.
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
