# Target Operations To Assigned Target Operations

[Pipeline](../pipeline.md) | Previous: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md) | Next: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md)

This stage assigns physical homes such as registers, stack slots, spills, and calling-convention locations.

## Stage Contract

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and calling-convention homes.

## Implementation Map

- `omega-target-operations/src/instruction/plan.rs` is the input
  representation root: executable target operation shape and host bindings live
  under `TargetOperationCode`, while preserved semantic evidence lives under
  `TargetSemanticSummary`.
- `omega-assigned-target-operations/src/plan.rs` is the output
  representation root: assigned executable shape and host bindings live under
  `AssignedTargetOperationCode`, while preserved semantic evidence lives under
  `AssignedSemanticSummary`.
- `builder.rs` owns the stage conveyor only: target identity, assigned code
  root, and preserved semantic root are assembled there.
- `code.rs` owns assigned executable-code root construction from target
  operation arenas and delegates runtime value home assignment to `values.rs`.
- `functions.rs` owns function metadata remapping while operation ordering is preserved.
- `operations.rs` owns target operation and instruction-operand conversion into assigned operation records.
- `instruction_operands.rs` owns assigned instruction operand records such as immediates, data addresses, and runtime string descriptors.
- `value_operands.rs` owns assigned runtime value operand records and their target/assigned handle bridge.
- `operation_conversions/` owns directional conversion between target operation kinds and assigned operation kinds.
- `semantics.rs` owns the assigned-stage semantic aliases.
  `AssignedSemanticSummary` is the preserved target/abstract semantic spine,
  not a new duplicate values/boundaries/ownership container. The stage should
  still assemble value, boundary, and ownership roots through the shared
  semantic-summary constructor so preservation remains explicit.
- `values.rs` owns runtime value operand home assignment, including stack/runtime homes and scratch-register selection.
- `registers.rs` owns architecture-specific scratch register selection until real allocation replaces the current fixed policy.
- `tests.rs` owns stage-level preservation canaries for value, ownership, and
  boundary policy-check metadata. Assigned operation and operand arenas retain
  the target arena identity used by the later exact callback registrar binding;
  this stage does not infer a binding from operand position.

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
