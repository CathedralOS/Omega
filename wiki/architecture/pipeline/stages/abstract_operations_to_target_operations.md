# Abstract Operations To Target Operations

[Pipeline](../pipeline.md) | Previous: [Control Flow To Abstract Operations](control_flow_to_abstract_operations.md) | Next: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md)

This stage legalizes abstract operations against target, layout, ABI, ISA, and calling-convention constraints.

## Stage Contract

Input: abstract operations.

Output: target-aware operations.

Primary responsibility: legalize operations using target, layout, ABI, ISA, and calling-convention knowledge.

## Implementation Map

- `translator.rs` owns the conveyor from abstract operation arenas into target operation arenas and preserves semantic metadata summaries across the stage boundary.
- `instructions.rs` owns abstract instruction legalization, including host-operation remapping into target operation keys.
- `operands.rs` owns instruction operand translation and abstract data-handle remapping.
- `values.rs` owns runtime value operand translation and runtime value handle remapping.
- `remap.rs` owns handle/span remapping when arena ordering is preserved across the lowering boundary.
- `host.rs` owns lowered host operation key resolution and host ABI binding reconciliation.
- `tests.rs` owns stage-level preservation canaries for values, ownership, and boundary edges.

## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Re-expressed as target-aware storage operands; no new language-level places are born here. |
| Values | Re-expressed as target value operands while abstract value summaries are preserved as target value metadata; this stage may choose target-legal operand shapes but should not invent semantic values. |
| Facts | Consumed only as already-lowered operation shape; proof and type facts are not re-proved here. |
| Loans | Not owned; borrow legality must already be decided before abstract operations exist. |
| Moves | Preserve abstract ownership summaries while target operations are legalized; explicit transfer operation lowering is still pending. |
| Drops | Preserve abstract ownership summaries while target operations are legalized; explicit cleanup operation lowering is still pending. |
| Calls | Host/runtime operation ordinals become target operation keys and ABI bindings. |
| Transitions | Preserved as target-aware branch/jump/return operations, not re-scheduled. |
| Effects | Carried through as concrete runtime/host operation choices. |
| Boundary edges | Preserve abstract source-boundary and host-operation boundary summaries while host operations resolve to ABI-aware operation keys and copied host bindings. |

## Ownership Rules

- Must preserve abstract operation order when remapping handles and spans.
- Must not own language acceptance of unsafe behavior, proof discharge, borrow checking, or effect authorization.
- Must keep legalization separate from physical register/stack assignment.

## Known Gaps

This stage still needs a richer distinction between target legalization, ABI lowering, and later physical assignment once target operations grow beyond the current direct mapping.
It also preserves ownership summaries without yet lowering them into target
copy/cleanup operations.
Value summaries are preserved through target legalization, but are not yet used
to drive target storage or ownership policy.
Boundary-edge summaries are preserved through target legalization, including
both source-level boundary edges and lowered host-operation edges.
