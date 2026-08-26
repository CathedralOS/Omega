# Control Flow To Abstract Operations

[Pipeline](../pipeline.md) | Previous: [State Graph To Control Flow](state_graph_to_control_flow.md) | Next: [Abstract Operations To Target Operations](abstract_operations_to_target_operations.md)

This stage starts Omega lowering. Today it adapts checked control flow into
abstract operations; in the target architecture it consumes terminal Psi
directly. See [Terminal Psi Architecture](../terminal_psi.md).

## Stage Contract

Current input: `ControlFlow`.

Target input: terminal Psi.

Output: target-independent abstract operations.

Primary responsibility: lower checked control flow into explicit operations with virtual registers and target-independent storage/value actions.

## Implementation Map

- `lib.rs` owns the public stage entrypoint only.
- `lowering.rs` owns abstract operation plan assembly. The transitional
  instruction-selection adapter still builds the executable code root, but this
  stage joins that code with preserved semantic evidence through
  `AbstractOperationPlan::with_roots` instead of mutating plan fields after
  construction.
- `lowering/input.rs` owns `AbstractOperationLoweringInput` and adapts the
  current control-flow/runtime planning bundle into instruction-selection input.
- `lowering/semantics.rs` owns construction of `AbstractSemanticSummary` from
  control-flow semantic roots and lowered host-call evidence. The top-level
  lowering code should assign this root as a unit instead of mutating individual
  semantic sub-arenas, and should use `AbstractSemanticSummary` constructors
  rather than spelling out its internal fields.
- `lowering/values.rs` preserves normalized arithmetic-policy adapter evidence
  in abstract value facts. During the transitional boundary, instruction
  selection consumes the control-flow copy for normalized float operations,
  validates the carried format against the selected width, and fails closed on
  conflicting evidence.
- `omega-control-flow/src/semantics.rs` is the source semantic root for this
  stage: `ControlFlowSemanticRoots` keeps proof, invariant, contract, value,
  boundary, borrow, and ownership arenas visibly separate from executable
  control-flow shape.
- `lowering/ownership.rs` owns the canonical control-flow permission-event copy
  into the abstract-operation ownership summary. Checked trees and subsequent
  IRs carry no parallel move/drop arenas. Each copied event retains its
  canonical control-flow arena identity so
  selection-time realization candidates can join without source-text identity.
- `lowering/boundary.rs` owns the host-operation to abstract boundary-edge
  summary copy. Before forming edges it independently replays one exact
  identity-only occurrence per `HostCallPlan` row: authored statement or
  expression handle, registrar target and canonical overload, state/statement/
  call ordinal, platform-lowering arena identity, and ordered formal ordinal to
  `NativeParameterId` rows. Result-storage pseudo-arguments are not formal
  arguments. Source boundary edges link only when state, statement, call
  ordinal, and resolved registrar target all agree. Each lowered edge points to
  its exact occurrence and records the operation ordinal inside the host call,
  so missing, duplicate, reordered, or drifted rows fail before target lowering.
  This carrier owns no physical place, address, byte offset, or relocation
  authority. At the first later backend-plan coexistence point, the private
  callback relocation demand joins this exact occurrence and one ordered native
  formal by typed arena handle. A nested callback destination retains its
  nominal layout identity and complete field path while selecting only the
  formal's root `NativeParameterId`; it still derives no physical offset or
  relocation authority.
- `omega-abstract-operations/src/plan/` owns the representation root:
  executable operation shape lives under `AbstractOperationCode`, while
  preserved semantic evidence lives under `AbstractSemanticSummary`.
  `plan/code.rs` owns the root structs and `plan/capacity.rs` owns capacity
  constructors.
- `omega-abstract-operations/src/semantics.rs` owns grouped semantic-root
  construction for abstract values, boundary edges, and ownership summaries.
  `instruction/function.rs`
  owns abstract function plans,
  `instruction/operation.rs` owns abstract operation records and source
  coordinates, `instruction/operation_kind.rs` owns abstract operation kinds,
  `instruction/value_operand.rs` owns abstract value operands, and
  `instruction/storage.rs` owns runtime storage regions.
- `omega-abstract-operations/src/data.rs` retains private dynamic-conformance
  table objects with their exact trait, conformance, and normalized row
  identities. Its unique lookup refuses missing, duplicate, and kind-drifted
  bindings. Transitional instruction selection consumes this table identity
  for the direct-place pass-through carrier: `WritePlaceAddress` materializes
  the instance word and `WriteDataAddressToRuntimeFrame` materializes only the
  table word. A failed dynamic join does not fall through to an ordinary copy.
- The actual operation construction currently happens in
  `omega-instruction-selection`; this is a transitional boundary, not the
  desired long-term split. Its instruction sink records exact source-site spans
  as permission-realization candidates. Abstract lowering publishes a ledger
  only when every canonical event has selected instructions or a validated
  no-code reason.
- Private dynamic-conformance rows are also reconstructed against checked
  normalized requirement/realization identity here. Each unique non-entry
  realization emits an ordinary private abstract function containing its exact
  retained control-flow state body; an entry realization reuses the existing
  entry function. Missing states, duplicate logical tables, or identity-to-key
  disagreement return a diagnostic before target lowering.

## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Lower toward abstract storage references, but much of that policy still lives beyond this adapter. |
| Values | Preserved as abstract value summaries; later passes should turn them into operands, temporaries, constants, virtual registers, or storage policy. |
| Facts | Preserved as diagnostic/proven metadata; not re-proved here. |
| Loans | Already validated; may remain as assertions or metadata. |
| Permission events | Canonical `Establish`, `Transfer`, `Consume`, and `AffineDrop` events are preserved with multiplicity, access, provenance, and live-obligation state; later lowering must realize each as an explicit transfer/cleanup action or a checked no-code case. |
| Calls | Should become abstract call operations. |
| Transitions | Should become branches, jumps, returns, exits, and block edges. |
| Effects | Should attach to abstract operations for later reporting/lowering. |
| Boundary edges | Control-flow source boundary edges and lowered host operations become distinct abstract boundary summaries beside abstract runtime/host/compiler calls. |

## Ownership Rules

- Must preserve checked/control-flow evidence while adapting into backend
  planning inputs.
- Must not own semantic proof discharge, borrow validation, target register
  assignment, machine instruction selection, object encoding, or final image
  policy.
- Must not hide long-term abstract-operation construction inside opaque adapter
  plumbing.

## Known Gaps

This stage is not yet a true representation-to-representation lowering pass.
Runtime and instruction-selection policy still owns too much of the abstract
operation construction that should eventually live here.
It also still receives `CheckedTrees` through transitional lowering input and
instruction selection performs binding substitution over source expression
tables. Terminal Psi moves concrete instantiation and expression lowering above
this boundary; the completed stage must consume no source-tree handles.
It preserves canonical control-flow permission events as abstract ownership
summaries and joins the currently covered runtime/direct selection sites to
their exact selected instructions. Folded materialization follows stable
establishment provenance. Explicit terminal consumes whose checked body emits
no instruction, no-live-debt events, and ordinary affine discard carry narrow
checked no-code reasons. Merely visiting an empty selection site is not proof
for a live establishment or transfer. Missing or
malformed coverage leaves the entire published realization ledger empty and is
reported as `INCOMPLETE`/`UNLINKED`. Dispatch transition edges now join their
argument-materialization instructions to target-state entry establishments, so
do state-call selection sites; runtime/direct state calls and statement-position
host calls retain their exact call ordinal. Named transition targets reserve
their canonical ordinal before nested argument calls, and transition-edge joins
also require the target symbol, so nested calls in one transition statement no
longer collide. Inline-branching state calls whose source operation defers code
selection join the eventual leaf-expansion span to the exact caller call event,
the called state's entry establishment, and the callee terminal event together.
A live obligation also remains available after a dispatched
call returns through its synthesized continuation; that edge preserves the
caller's place/provenance rather than creating a new permission event. The
same-target transition-call canary also proves that ordinal identity separates
two calls that share one target symbol and that both materializations join the
shared target-state event. Program-entry StateEntry events join the normalized
platform argument writes before either selection path begins; missing inbound
code stays unlinked, and a later consume cannot stand in for establishment. The
complete current ownership pass corpus is covered. Remaining gaps include
constructing and lowering checked `EdgeCleanupPlan` actions and their
conservation witness for state exits, plus ownership forms not reached by
current operation-site hooks.
It preserves control-flow value summaries as abstract value summaries.
Normalized float runtime-operand lowering consumes the carried checked provider
identity and policy adapter and fails closed when either fact is absent or
contradictory. The summaries do not yet decide type-aware ownership kind or
storage shape.
Boundary-edge summaries now preserve source-level boundary trait edges, exact
identity-only outbound host-call occurrences and native formal identities,
lowered host-operation edges, and target-aware links between those layers.
Executable host operations still use transitional coarse source coordinates;
moving their exact occurrence handle into the operation stream remains a later
representation cleanup, separate from physical callback placement.
