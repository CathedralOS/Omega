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
- `validation/catalog/dispatch/mod.rs` maps that inventory through immediate,
  parameter, and terminal adapters. Parameter adapters descend through
  `parameter/{direct,unary,bitwise,comparison,arithmetic,shift}` stage groups.
- `validation/straight_line_integer_immediate.rs` owns the first closed
  translation family: parameterless `[IntegerConstant, Return]` to
  `ReturnIntegerImmediate`.
- `validation/straight_line_integer_widen_immediate/mod.rs` owns the adjacent
  exact constant-materialization family. Its grammar independently
  reconstructs parameterless `[IntegerConstant, IntegerWiden, Return]`; target
  replay proves that lowering materialized the mathematical widened value as
  `ReturnIntegerImmediate` while retaining both source definitions and their
  ordered provenance.
- `validation/straight_line_boolean_immediate.rs` owns its exact sibling:
  parameterless `[BooleanConstant, Return]` to `ReturnBooleanImmediate`.
- `validation/straight_line_scalar_crash.rs` owns parameterless one-block
  scalar `[Crash]` to exact target `Crash` custody.
- `validation/straight_line_parameter/mod.rs` owns the shared source-envelope
  to native-ABI replay join for nonempty scalar parameter rosters. Boolean and
  integer target replay descend through named `direct`, `unary`, `bitwise`,
  `comparison`, `arithmetic`, and `shift` folders.
- `validation/straight_line_parameter/source/mod.rs` maps source grammar,
  descending into a common `envelope.rs`, direct-return and Boolean grammar,
  or `source/integer/mod.rs`. The integer coordinator owns common
  typed-parameter lookup before descending into comparison, unary, bitwise,
  arithmetic, or independently typed shift grammar.
  Unary source replay distinguishes bitwise-not from widen and validates the
  exact fixed-integer widening relation before target replay begins.
- `validation/straight_line_parameter/{boolean,integer}/{direct,unary,bitwise,comparison,arithmetic,shift}`
  retain distinct exact family identities and validate corresponding target
  variants after independent register or stack reconstruction. Binary
  Boolean-result families open recursive `ReturnBooleanExpression` carriers
  and retain ordered or identical operands; integer equality, less-than, and
  less-or-equal also bind their common exact integer type. Integer bitwise-not
  opens `ReturnIntegerExpression::BitwiseNot`, retaining its exact type and
  parameter operand. Integer widen opens
  `ReturnIntegerExpression::IntegerWiden`, retaining distinct source and target
  types and accepting only same-sign or unsigned-to-larger-signed native
  fixed-integer widening. Integer bitwise AND, OR, and XOR each retain ordered
  or identical operands and the exact common fixed-width integer carrier;
  shared ABI/provenance replay sits below their operator-specific leaves.
  Wrapping shift-left and shift-right descend through a distinct shift rung
  that retains independently typed value/count operands and ABI locations;
  right shift preserves unsigned fixed/address zero-fill and signed fixed
  sign-fill after canonical modulo-width count reduction.
  Proof-bearing exact shifts share that carrier only after independently
  rejecting address value/count types. Exact shift-right retains its canonical
  count-range obligation; exact shift-left retains the stronger canonical
  count-range and mathematical-result-representability obligation through the
  target expression and receipt.
- `validation/model/{error,receipt}/mod.rs` are the small family maps above
  immediate, terminal, roster, and parameter-specific vocabulary leaves.
- `conditional_control.rs`, `conditional_scalar.rs`, `structural_result.rs`, and
  `structural_scalar.rs` own their corresponding closed operation families.
- `lowering/scalar_abi.rs` derives the exact canonical target ABI for the
  bounded service-free fixed-integer function family. `lowering/unit/scalar_call.rs`
  admits attached-Unit calls only when that independently derived callee ABI
  matches, retaining constants or earlier call results as typed sources.
- `lowering/coordination.rs` consumes one exact admitted nearest-FMA settlement
  for every Abstract FMA occurrence. `lowering/unit.rs` retains raw
  binary32/binary64 operands, the selected-plan commitment, slot, and provider
  in target operations without choosing physical registers. The bounded
  attached-Unit lane may preserve ordinary receiver-attached zero-result
  internal Unit calls or the bounded zero-argument, zero-result
  source-evaluated foreign leaf after those FMA definitions; their existing
  call custody remains distinct.
- `lowering/unit/dynamic_scalar.rs` independently rejoins one rebound dynamic
  descriptor with its exact initializer, latest source, indirect dispatch, and
  selected realization. It derives the realization's structural argument and
  scalar result ABI while preserving both source copies; physical assignment
  later allocates the canonical two-word descriptor and durable result home.
- `lowering/unit/conditional_exit.rs` owns the exact attached-Unit equality
  diamond used by a rebound dynamic result followed by two admitted
  nonreturning boundary leaves. It preserves all ten Terminal operation
  ordinals with explicit zero-code control markers and carries both successor
  edges; it is not a general CFG legalization path.
- `lowering/unit/projected_argument.rs` owns the shared target-layout lowering
  for one structural argument projected from an attached Unit parameter.
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
- An attached-Unit scalar call may request a result home, but this stage does
  not choose its byte offset; assignment owns the ordered physical layout.
- Nearest-FMA legalization must retain exact occurrence/plan/admission custody,
  but assignment—not target legalization—owns its XMM register choices.
- Internal Unit calls in an FMA-bearing body remain in source order inside the
  function-level canonical floating-control envelope; later object replay
  checks their complete emitted intervals against that envelope.
- A normalized foreign locator's exact target profile owns locator
  applicability. Target lowering preserves that sealed locator and checks its
  native target; it does not maintain a second format/case allowlist.

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
Rebound dynamic calls now cross target legalization, physical assignment,
selected-conformance table materialization, indirect-call encoding, and object
relocation on x86-64 and AArch64. The exact three-block equality/exit
continuation used by the direct rebound canary also reaches machine code and
object replay. General attached-Unit CFG legalization remains open outside
that explicitly bounded family.
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
