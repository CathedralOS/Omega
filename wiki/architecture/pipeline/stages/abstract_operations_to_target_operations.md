# Abstract Operations To Target Operations

[Pipeline](../pipeline.md) | Previous: [Control Flow To Abstract Operations](control_flow_to_abstract_operations.md) | Next: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md)

This stage legalizes abstract operations against target, layout, ABI, ISA, and calling-convention constraints.

## Stage Contract

Input: abstract operations.

Output: target-aware operations.

Primary responsibility: legalize operations using target, layout, ABI, ISA, and calling-convention knowledge.

## Implementation Map

- `translator.rs` owns the conveyor from abstract operation arenas into target operation arenas and preserves semantic metadata summaries across the stage boundary.
- `semantics.rs` owns target semantic summary assembly. It should join value,
  boundary, and ownership roots through the shared semantic-summary constructor,
  with boundary policy validation isolated to the boundary root.
- `instructions.rs` owns abstract instruction legalization, including host-operation remapping into target operation keys.
- `operands.rs` owns instruction operand translation and abstract data-handle remapping.
- `values.rs` owns runtime value operand translation and runtime value handle remapping.
- `remap.rs` owns handle/span remapping when arena ordering is preserved across the lowering boundary.
- `host.rs` owns lowered host operation key resolution and host ABI binding reconciliation.
- `boundary_policy.rs` owns first-pass target boundary validation: it records
  whether each lowered host operation is linked to a source boundary edge and
  whether the target ABI has a binding/policy for that operation. It also
  checks that the binding policy is allowed by the selected target ABI policy
  set. Linked-edge discovery, unlinked-edge discovery, and policy-check record
  construction should stay separate so later diagnostics can grow without
  re-matching boundary summaries by hand.
- `omega-target-operations/src/instruction/function.rs` owns target operation function plans.
- `omega-target-operations/src/instruction/operation.rs` owns target operation records and source coordinates.
- `omega-target-operations/src/instruction/operation_kind.rs` owns target operation kinds.
- `omega-target-operations/src/instruction/abstract_conversions.rs` owns abstract-operation to target-operation conversion.
- `omega-target-operations/src/instruction/plan.rs` owns the representation
  root: executable target operation shape and host bindings live under
  `TargetOperationCode`, while preserved semantic evidence lives under
  `TargetSemanticSummary`.
- `omega-target-operations/src/instruction/semantics.rs` owns the target-stage
  semantic aliases. `TargetSemanticSummary` is the preserved abstract semantic
  spine, not a second copy of the same values/boundaries/ownership shape.
- `omega-target-operations/src/instruction/value.rs` owns target value operands.
- `omega-target-operations/src/instruction/operand.rs` owns target instruction operands.
- `tests.rs` owns stage-level preservation canaries for values, ownership,
  boundary edges, and the exact identity-only outbound host-call/native-formal
  catalogs. Target legalization preserves those catalogs unchanged and does
  not reinterpret them as ABI placement or relocation authority.

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
| Boundary edges | Preserve abstract source-boundary, exact host-call occurrence/native-formal, host-operation boundary, and source-to-lowered link summaries while recording target policy checks for linked, unlinked, and unbound host operations. |

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
source-level boundary edges, exact registrar occurrence/native-parameter
identity rows, target-aware links, and lowered host-operation edges. These rows
remain address-free; target physical destinations and object relocations are
not inferred here.
Boundary policy checks currently validate source-link presence, target host
binding presence, and whether the binding policy is allowed by the selected ABI
policy set. Exact source policy path matching is still pending because source
boundary policy paths are not yet represented in the semantic spine.
