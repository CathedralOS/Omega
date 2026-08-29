# Abstract Operations To Target Operations

[Pipeline](../pipeline.md) | Previous: Terminal Psi To Abstract Operations | Next: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md)

This stage legalizes abstract operations against target, layout, ABI, ISA, and calling-convention constraints.

## Stage Contract

Input: abstract operations.

Output: target-aware operations.

Primary responsibility: legalize operations using target, layout, ABI, ISA, and calling-convention knowledge.

## Implementation Map

- `omega-abstract-operations-to-target-operations/src/lib.rs` owns the stage
  responsibility map.
- `lowering/mod.rs` owns settlement and common legalization dispatch.
- `validation/mod.rs` independently binds target, roots, and the complete
  function roster before dispatching exact semantic-family replay.
- `validation/catalog/mod.rs` is the sole ordered enable/disable inventory for
  translation families. Its descriptor and dispatch leaves connect one source
  classifier to one typed validator, and ambiguous classification fails closed.
- `validation/straight_line_integer_immediate.rs` owns the first closed
  translation family: parameterless `[IntegerConstant, Return]` to
  `ReturnIntegerImmediate`.
- `validation/straight_line_boolean_immediate.rs` owns its exact sibling:
  parameterless `[BooleanConstant, Return]` to `ReturnBooleanImmediate`.
- `validation/straight_line_scalar_crash.rs` owns parameterless one-block
  scalar `[Crash]` to exact target `Crash` custody.
- `conditional_control.rs`, `conditional_scalar.rs`, `structural_result.rs`, and
  `structural_scalar.rs` own their corresponding closed operation families.
- `omega-target-operations/src/lib.rs` owns the target-aware output vocabulary.
- `tests.rs` owns exact stage-boundary and rejection canaries.
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
| Boundary edges | Preserve abstract source-boundary, exact host-call occurrence/ordered native-parameter telescope, host-operation boundary, and source-to-lowered link summaries while recording target policy checks for linked, unlinked, and unbound host operations. |

## Ownership Rules

- Must preserve abstract operation order when remapping handles and spans.
- Must not own language acceptance of unsafe behavior, proof discharge, borrow checking, or effect authorization.
- Must keep legalization separate from physical register/stack assignment.

## Known Gaps

This stage still needs a richer distinction between target legalization, ABI lowering, and later physical assignment once target operations grow beyond the current direct mapping.
Independent translation validation is intentionally incremental. Its receipt
lists the exact functions covered by implemented family validators rather than
claiming whole-plan semantic validation for unmatched functions. Root, target,
entry, function-order, machine, and attachment custody already cover every
plan; additional operation families must add adjacent independent replay rows.
It also preserves ownership summaries without yet lowering them into target
copy/cleanup operations.
Value summaries are preserved through target legalization, but are not yet used
to drive target storage or ownership policy.
Boundary-edge summaries are preserved through target legalization, including
source-level boundary edges, exact registrar occurrence/native-parameter
identity rows, target-aware links, and lowered host-operation edges. These rows
remain address-free. The opted-in registrar host-operation provenance adds
exact source-call, occurrence, edge, ordinal, and operand handles, but target
physical destinations and object relocations are not inferred here.
Boundary policy checks currently validate source-link presence, target host
binding presence, and whether the binding policy is allowed by the selected ABI
policy set. Exact source policy path matching is still pending because source
boundary policy paths are not yet represented in the semantic spine.
