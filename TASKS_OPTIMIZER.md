# Optimizer tasks

This is the optimizer execution board, not its history. Architecture and
rationale live in
[optimizer implementation](omega-rust/optimization.md),
and landed milestones live in Git. Remove work from this file when its
acceptance condition passes. Tag an added item's provenance on its first line:
`(new-scope)` for newly discovered work, `(split-of:<parent-item>)` when it
decomposes an existing item. Items added before 2026-09-14 are untagged.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias. Every rule must name the exact semantic, proof, ownership,
effect, target, and provenance facts it consumes, and retain the identities
needed for independent replay through publication.

## Pipeline cleanup follow-ups

These are separately schedulable follow-ups, not an ongoing cleanup run.
The [ownership contract](omega-rust/pipeline.md)
governs all three: replace obstructive implementations, preserve semantics and
independent validation, and keep empty/nonempty optimization selections on one
physical route. Unsupported cases reject rather than restoring a fallback.

- **PIPELINE-OWNER-CONSOLIDATION.** Finish ownership in
  `omega-rust/{omega,psi}/pipeline/` and their compiler/backend coordinators.
  Merge or delete umbrella/helper owners; put transformations in literal
  `X-to-Y` stages and optimizations in `X-to-X` stages.
  Acceptance: folders expose the connected program sequence, no competing
  entrances or orphan outputs remain, and coordinators only sequence typed
  stages. Renaming a helper or adding a wrapper is not completion.

- **REPRESENTATION-OWNERSHIP.** Finish
  `omega-rust/{omega,psi}/representations/`: one named program-root file beside
  `lib.rs`, with concept-owned subdirectories and current data independent of
  producer history. Move durable schemas out of transforms/backends where
  necessary, alongside their consuming stage changes.
  Acceptance: current programs outlive their producers; ordinary consumers
  read current data directly; historical inputs remain separate replay evidence.

## Product pruning and rollout

- **WORKSPACE-ROLLOUT.** Keep every rule explicit opt-in until the frozen-tree
  command `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace
  --no-fail-fast` passes. Do not replace it with `--all-targets`, which omits
  doctests. Acceptance: the command passes from a clean checkout and every
  promoted exact rule has its required rollout evidence.

## Validation, translation, and publication

- **TRANSLATION-VALIDATION.** Complete independent source-to-target replay for
  admitted operations and transfers on the common graph. Scalar expression-family
  planners, Unit/structural whole-function templates, their catalogs and
  compatibility fixtures are retired: do not restore
  them to recover arithmetic, crash, cleanup or borrowed-call coverage. Extend
  ordinary graph operations and their receiving checks instead. Remaining native
  operations and publication routes fail closed until target applicability,
  result, effect, cleanup, proof and custody facts reconstruct.

  Close [physical-child replay](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence)
  with one source-reachable selected-lowering operation carrying
  nonempty physical evidence through allocation, layout, native emission, and
  independent replay. Bind the immutable Terminal product, validated
  optimization projection, and each surviving boundary occurrence to exactly
  one physical child with its operator-application or boundary-settlement parent.
  Acceptance: post-Psi optimization
  and final publication reject missing, duplicate, stale, substituted, padded,
  or role-swapped children; only verified eliminated occurrences need no child.
  General calls depend on `FRAME-LAYOUT` and `GENERAL-CALL-CLOBBERS` below.
  These are native compiler guarantees, independent of package locks or
  `PackageInstance` construction. One source-reachable
  selected-lowering operation now carries nonempty physical evidence through
  the common physical stages to native emission and independent replay —
  `optimizer_opt_in.rs::
  selected_lowering_boundary_occurrence_replays_one_exact_physical_child`
  compiles a package-bound `boundary operator` program under
  `SelectedIncomingU12CompareImmediate`, replays the artifact, and binds
  exactly one `OperatorApplicationCoverage` physical child (nonempty
  machine/object spans, `ResolvedInternalCall` relocation) to the one
  surviving operator occurrence and the validated projection identity. The
  same test replays mutated custody: missing, duplicated, role-swapped
  (operator→boundary), padded-span, and substituted-projection children all
  reject. `selected_lowering_replays_one_physical_child_per_surviving_occurrence_role`
  retains both surviving roles in one program — the closed operator
  application and a hosted console exit — and binds each surviving
  occurrence to exactly one physical child: `OperatorApplicationCoverage`
  for the operator occurrence and `BoundaryTraitSettlement`
  (`HostedExitProcessI32`, `DirectInstructionBytes`) for the boundary
  occurrence; dropping either role, duplicating a child, or swapping a
  child's parent all reject on independent replay.
  `verified_eliminated_occurrence_needs_no_physical_child` closes the
  child-exemption half: two private machines each apply a covered boundary
  operator while a constant-false transition arm keeps the dead callee
  source-reachable through Terminal admission, so checked D29 coverage
  names both operations and the ordinary build binds each surviving
  occurrence to its own child. Replaying the published artifact sections
  through `optimize_verified_abstract_input` under `ControlFlowCleanup`
  folds the dead arm and proves the dead callee unreachable — pruned
  machine custody plus a `ProvenUnreachableAt` ledger row for the exact
  eliminated call node — and the validated optimized projection keeps
  only the surviving occurrence. The same coverage over the identity plan
  still projects both occurrences, so the exemption attaches to the
  verified elimination rather than the coverage row; a replayed physical
  child bound to the eliminated occurrence's canonical identity rejects,
  and dropping a still-required child rejects too.

- **CUSTODY-MUTATION-COVERAGE.** Complete authenticated one-field mutation
  tests for every remaining manifest, receipt, codec, and artifact-custody
  family. Acceptance: each representable field can be changed independently,
  the containing identity can be recomputed, and independent replay still
  rejects the substitution. Landed: installed function rows
  (`image-emission/tests/artifacts.rs`,
  `installation_function_row_rejects_every_one_field_substitution`) — machine,
  attachment, unit stack, and scalar stack are independently representable and
  rejected by replay; text offset, byte count, dropped rows, and the unit-body
  flag are rejected at encoding as non-canonical. Landed: installed internal
  unit call rows
  (`installation_internal_unit_call_row_rejects_every_one_field_substitution`)
  — the cleanup-owned call's byte count is independently representable and
  rejected by replay; every other field on operation- and cleanup-owned rows,
  dropped rows, and the semantic-result projection are rejected at encoding
  as non-canonical. Landed: installed boundary-settlement rows
  (`installation_boundary_settlement_rejects_every_one_field_substitution`)
  — `psi_operation`, `boundary`, the write row's `operation_ordinal`, the
  admitted-provider execution identity, byte-sequence `literal_operation` and
  structural-type identity, scalar-argument `source_value` and `immediate`
  are independently representable and rejected by replay; machine,
  text/code offsets, byte counts, realization/execution swaps, argument
  rosters, completion custody, native results, the exit row's ordinal, a
  duplicate `psi_operation`, and roster reorder are rejected at encoding as
  non-canonical; a dropped row is rejected by replay. Landed: installed
  compiler-private callback rows
  (`installation_private_function_row_rejects_every_one_field_substitution`)
  — callback-thunk continuation and placement index, source-Psi fingerprint,
  machine, and scalar-ABI parameter/result values are independently
  representable and rejected by replay; a non-thunk identity kind, text
  offset, zero/extended byte count, non-canonical ABI placements and value
  collisions, and a dropped row are rejected at encoding as non-canonical.
  Landed: installed dynamic conformance tables and dynamic call rows
  (`installation_dynamic_conformance_table_rejects_every_one_field_substitution`)
  — the application report fingerprint, an unresolved slot's target, and the
  call's operation, text offset, and byte count are independently
  representable and rejected by replay; the application commitment, a zero
  fingerprint, table data offset and byte count, slot row indices and data
  offsets, resolved-slot retarget or erasure, unknown slot targets, the
  caller machine, initial and rebound sources, the selected table byte
  offset, the realization, and swapped, dropped, or duplicated slots,
  tables, and calls are rejected at encoding as non-canonical. Landed:
  installed dynamic-parameter call rows
  (`installation_dynamic_parameter_call_rejects_every_one_field_substitution`)
  — `operation`, `source_value`, `requirement_slot`, in-function text offset,
  and in-bounds byte count are independently representable and rejected by
  replay; an unknown or other-function machine, out-of-function text offsets,
  zero or overflowing byte counts, and a duplicated row are rejected at
  encoding as non-canonical; a dropped row is rejected by replay. Landed:
  installed stored dynamic call rows
  (`installation_stored_dynamic_call_rejects_every_one_field_substitution`)
  — the table's application report fingerprint and an unresolved slot's
  target, the dispatch's operation and text offset and byte count, the
  establishment's operation and byte count, and the descriptor and
  selection ordinals and descriptor home offset are independently
  representable and rejected by replay; the application commitment, caller
  machine, unselected realization, unknown source place, selected table
  byte offset, empty or out-of-function byte counts, and out-of-range or
  overlapping establishment and dispatch offsets are rejected at encoding
  as non-canonical, as are dropped, duplicated, or swapped
  conformance-table slots and dropped or duplicated calls and tables.
  Landed: installed forwarded dynamic-parameter call rows
  (`installation_forwarded_dynamic_parameter_call_rejects_every_one_field_substitution`)
  — the call's operation, source value, scalar type, self-function callee,
  text offset, and byte count are independently representable and rejected
  by replay; an unknown or other-function machine or callee, parameter
  ordinals, a cleared or non-integer scalar type, out-of-function text
  offsets, an empty byte count, and a duplicated row are rejected at
  encoding as non-canonical; a dropped row is rejected by replay. Landed:
  installed forwarded dynamic-descriptor adapter, table, and call rows
  (`installation_forwarded_dynamic_descriptor_rejects_every_one_field_substitution`)
  — the table's application report fingerprint, and the call's operation,
  other-function callee, source place and access, text offset, and byte
  count are independently representable and rejected by replay; adapter and
  table application commitments, adapter row index, realization, text
  offset and byte count, table data offset and byte count, slot row index,
  realization and offset joins, the call's machine, callee, commitment and
  semantic result, and out-of-range offsets and byte counts are rejected at
  encoding as non-canonical, as are dropped or duplicated adapters, tables,
  and calls. Landed: installed semantic code attribution rows
  (`installation_semantic_code_attribution_rejects_every_one_field_substitution`)
  — operation- and edge-site identities and in-bounds byte counts are
  independently representable and rejected by replay; an unknown machine, a
  roster-reordering ordinal, drifted code and text offsets, a past-function
  byte count, and the boundary-joined return-edge row's site, ordinal, and
  byte count are rejected at encoding as non-canonical; a dropped unjoined
  row is rejected by replay while a dropped return-edge row, a duplicated
  site, and a reordered roster are rejected at encoding. Landed: installed
  privileged port-effect rows
  (`installation_port_effect_rejects_every_one_field_substitution`)
  — an unbound row's operation, service, port, value, and ordinal are
  independently representable and rejected by replay; its machine, code
  offset, byte count, and text offset are rejected at encoding as
  non-canonical; every field of the `MetadataOnlyPort`-consumed row is
  rejected at encoding through the canonical offset joins or the settlement
  realization; a dropped unbound row is rejected by replay while a dropped
  consumed row, a duplicated `(machine, operation)` pair, and a reordered
  roster are rejected at encoding. Landed: installation header and manifest
  axes (`installation_header_rejects_every_one_field_substitution`) — the
  program fingerprint, architecture, whole-target, and bound image
  fingerprint substitutions, the image-section layout addresses, the retained
  compiler text-validation receipt, and its report-only derivation
  fingerprint are independently representable and rejected by replay, as is
  every derivation-digest input once the receipt identity is honestly
  recomputed; the profile decision and the committed component-progress
  manifest and acceptance identities sit outside the image join and break
  the published record identity a deployment journal replays; a retained
  subsystem, unsupported target facts, non-canonical section projections,
  and receipt fields without a consistent derivation digest are rejected at
  encoding as non-canonical, while the COFF subsystem value is rejected by
  replay and its absence or a non-COFF target at encoding; the magic,
  format, and vocabulary markers, unknown enum tags, the reserved field,
  zero profile and progress identities, and presence-flag lies reject at
  the wire. Landed: selected provider-plan closure rows
  (`installation_selected_provider_plan_rejects_every_one_field_substitution`)
  — substituting, extending, or dropping an unexecuted selected plan is
  independently representable, produces the record an honest admission of
  the mutated closure builds, and breaks the published record identity a
  deployment journal replays while the image join cannot see it;
  substituting or dropping the executed plan, clearing the roster,
  reordering it, or duplicating an entry is rejected at encoding through
  the settlement-closure and canonical-order joins, a reported execution
  outside the selected closure or a reported closure diverging from the
  image's retained executions is rejected at admission, and a zero
  identity, a non-canonical or duplicated wire order, and an uncarried
  count reject at decode. Landed: installed structural-return rows
  (`installation_structural_return_rejects_every_one_field_substitution`)
  — the affine and linear rows' result places and the linear row's carried
  claim identity are independently representable and rejected by replay;
  machine, `psi_edge`, scalar and structural parameters, placements, source
  and result signature fields, affine-lane claims, trivial locals and
  discards, code offset, and byte count are rejected at encoding as
  non-canonical, as are a machine-descending swap and a duplicated row,
  while a dropped row is rejected by replay. Landed: installed attached-Unit
  scalar-call rows (`image-emission/tests/internal_unit_scalar_calls.rs`,
  `installation_internal_unit_scalar_call_row_rejects_every_one_field_substitution`,
  `951c09bb9c`) — the call's `operation_ordinal`, in-span `byte_count`, the
  nested argument and result `code_offset`/`byte_count` intervals, and a
  `Home` argument source naming an earlier producer are independently
  representable and rejected by replay, as are dropped rows; machine, text
  offset, owner, target, call-plan parameters/result/stack alignment, every
  result-home field, the result source, out-of-span or zero intervals,
  constant or out-of-order ordinals, argument parameter index, destination
  and source substitutions, extra or dropped arguments, and duplicated or
  swapped rows are rejected at encoding as non-canonical. Landed: sealed
  optimized-object artifact records, manifests, and custody receipts
  (`compiler/tests/object_artifact_custody.rs`,
  `optimized_object_artifact_custody_rejects_every_one_field_substitution`)
  — every artifact-record field (terminal-artifact, semantic, obligation,
  and proof digests, the debug section, selections, all four target axes,
  semantic entry, the six manifest identities, object and container
  identities, and all four statistics) and every manifest field (artifact,
  terminal-artifact, semantic, selections, all four target axes, semantic
  entry, container-manifest, object, container, and all four statistics)
  is independently representable under an honestly recomputed containing
  identity and rejected by independent replay against the retained terminal
  artifact and relocation-free object container; each of the six
  custody-receipt fields is rejected by replay; foreign or stale containing
  identities, the closed stage and unavailable markers, unknown vocabulary,
  architecture, object-format, and optional-section tags, a zero machine
  identity, and trailing or truncated envelopes are rejected at canonical
  decoding.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Complete ordinary cyclic Terminal Psi execution
  with authenticated SCCs, dominance and
  frontiers, optional well-founded ranking, productive unranked components,
  and structured finite-work failures. The dedicated unsigned-countdown native
  carrier is removed; its old custody rejects. Extend the common graph and
  ordinary ranking evidence rather than restoring a second native route.

  First bounded call-composition milestone: implement the
  [ranked callee on a projected receiver](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers),
  beyond the current whole-entry-only admission. Acceptance: an ordinary caller
  borrows a nested field, its ranked callee preserves that referent across
  backedges, and the caller observes writes after return. Conflicting parent
  access and missing/invalid callee ranking reject. Validate argument identity,
  call/return, cleanup, and composed resource bounds through native replay and
  execution on both Linux architectures. This needs no new receiver syntax.
  Broader changing-reference transfers still require loan/alias and ranking
  substitution work; increasing the parameter count alone does not close them.

- **TERMINAL-SCC-CONSUMERS.** Retarget LICM and other loop consumers from the
  exact countdown slice to validated Terminal SCCs. General invariant
  discovery, profitability, and motion remain open.

- **GENERAL-LICM.** Implement motion only through transformations that
  invalidate and reconstruct component, loop-carried custody, ranking,
  provenance, effect, and fuel evidence. The dedicated countdown zero/one
  relocation is not general LICM authority. The unique-entry preheader
  boundary relocates scalar constant leaves and side-effect-free scalar
  computations, and its invariant discovery resolves member parameters
  transitively across component-internal edges to the representative every
  reaching edge agrees on, rebinding moved operands to the
  preheader-visible anchor. Remaining: non-scalar families, profitability,
  and motion beyond the unique-entry preheader.

## Lowering and instruction selection

- **EXACT-SELECTION-FAMILIES.** Add address-mode folding, compare/branch
  selection, extension elimination, and constant materialization one exact
  named family at a time. Each family needs a disjoint source grammar,
  independent validator, target applicability, corruption controls, and
  publication replay. Resume evidence (verified at `8c0d4dbb45`, Linux
  x86-64): the only existing family seam is the selected-lowering literal
  fold in `selected-instructions-to-selected-instructions`
  (`rewrites/selected_lowering/literal_fold`). It is not a general peephole:
  it fires only from a pressure-recovery
  `ImmediateU64RematerializationCandidate` with `Incoming` role, folds one
  `MaterializeI64` into the `[left, result]` consumer at operand 1 with a
  `Def` result and u12 immediate, and both `SelectedInstructionPairRule` and
  the inline validator (`validate/replay.rs`) assume that shape. Compare with
  a zero literal is already selected directly as `CompareI64Zero`
  (`target-operations-to-selected-instructions` `zero_compare.rs`), so no
  compare/zero fold has a customer. The next compare/branch family
  (`CompareI64` + u12 literal → `CompareI64Immediate`) needs a new
  `SelectedInstructionKind` across `selected-instructions`, both ISA encoders,
  `register-environment`, selection identity, and the optimizer vocabulary,
  plus a fold action for flag-defining consumers without a `Def` result.
  Landed: the `CompareI64Immediate` kind and both encoders (x86-64 `cmp
  r64,imm32`, AArch64 `subs xzr,xN,#imm12`; immediate domain is u12, semantic
  tag 53), with canonical zero decode on AArch64 reusing `CompareI64Zero`,
  and the compare fold family itself: `Optimization::
  SelectedIncomingU12CompareImmediate` (tag 21) enables the
  `COMPARE_IMMEDIATE_U12` pair rule, the flag-defining/no-`Def` fold action
  (`LiteralFoldAction.result: Option`, codec v4), flag-shape immediate-row
  validation in compute and independent replay, and firing/corruption/replay
  coverage on both Linux targets. Selected-lowering selections now
  execute through the common physical stages — the staging gate resolves the
  exact catalog instead of rejecting the phase
  (`native-realization/.../physical_pipeline/phase_selections.rs`), and fixed-frame
  realization binds the completion identity and literal-fold transformation
  ledger into the expected manifest
  (`machine-emission/.../assembly/fixed_frame.rs`). Add, subtract, and compare
  return-only builds replay to `ValidatedOptimizedProjection`, and a
  boundary-operator program replays one exact physical child.
  Landed: the extension-elimination family —
  `Optimization::SelectedIncomingLiteralExtensionElimination` (tag 23)
  enables `EXTENSION_V1` and the six `EXTENSION_LITERAL_FOLDS` unary pair
  rules (`MaterializeI64` + `ZeroExtendU8`/`U16`/`U32` or
  `SignExtendI8`/`I16`/`I32` → `MaterializeI64` of the folded output bits).
  The catalog payload now names a slice of pair rules per selection, and
  `PairOperandShape` (binary right-literal vs unary sole-operand) is a third
  declared rule dimension beside result disposition and the unit-effect
  surface. The independent replay re-derives the grammar, folded bits, and
  materialized `IntegerValue` from the consumer kind and the surviving
  result register's scalar type; a `windows_x86_64` return-only build
  replays to `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_extension_elimination.rs`). Landed: the
  constant-materialization family —
  `Optimization::SelectedIncomingLiteralCopyMaterialization` (tag 25)
  enables `COPY_V1` and the `COPY_LITERAL_FOLD` unary pair rule
  (`MaterializeI64` + `CopyI64` → `MaterializeI64` of the literal itself at
  the copy's destination). The fold admits the full u64 literal domain — no
  narrowing — so the copy's result register must admit the literal. The
  independent replay binds the shared `MaterializeI64` constraint row under
  a second policy-gated slot so a copy fold cannot replay under the
  extension selection alone; a `windows_x86_64` return-only build replays to
  `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_copy_materialization.rs`). The deferred
  `phase_selections/tests.rs` enable-list addition landed with it
  (indexed-load and copy rows both listed now). Landed: the address-mode
  folding family —
  `Optimization::SelectedIncomingU12ByteViewAddressOffset` (tag 26)
  enables `BYTE_VIEW_ADDRESS_V1` and the `BYTE_VIEW_ADDRESS_OFFSET_U12`
  pair rule (`MaterializeI64` + `ByteViewAddress` → `AddressOffset` of the
  folded offset): the projection's operand-1 `Use` is the folded literal,
  its operand-0 `Use` base survives, and its operand-2 `Def` is the result.
  Both forms are effect-isolated on x86-64 and AArch64, so the rule rides
  the ordinary binary-right-literal grammar and isolated effect surface;
  the declared u12 bound is the aarch64 `add`-immediate encoding limit.
  The independent replay binds the `AddressOffset` row under its own
  policy-gated slot and rebuilds the constant-offset form from the
  consumer kind alone; a `windows_x86_64` return-only build replays to
  `ValidatedOptimizedProjection`
  (`compiler/tests/optimizer_byte_view_address_offset.rs`).
  Remaining: none of the originally suggested families are outstanding;
  any further family follows the same one-exact-named-family-at-a-time
  contract.

- **SELECTED-ABI-VALIDATION.** Validate ABI operands, calls, clobbers, effects,
  traps, provenance, cleanup, and logical fuel across every selected rule.
  The effect catalog now pins every encoded footprint to its owning
  semantic: `validate_declaration` binds the declared trap surface —
  hosted trap results name their owning hosted operation, and a
  never-faulting declaration cannot also declare a memory access — and
  `validate_encoded_effects` binds each row's shape, so control-flow
  rules pin their exact encoded control inside the barrier class,
  returns and calls can no longer borrow each other's activation-stack
  and return-address lifecycle rows, indexed-pointer and frame-storage
  rows reject foreign semantics, and the plain fallthrough row admits no
  hosted trap shape (crate `nextest`: 41 pass, including
  cross-borrowing, foreign-semantic, trap-understatement, and
  catch-all-evasion negatives; 234 isa-x86_64, isa-aarch64, and
  register-environment lib tests confirm every real catalog row still
  validates on Linux x86-64). Remaining: provenance, cleanup, and
  logical-fuel dimensions, and deeper per-rule operand/clobber coverage;
  Windows and macOS runs were unavailable on this host.

## Register allocation and frames

- **SPILL-REALIZATION.** Extend executable spill recovery beyond dominating
  nonaddress instruction results in acyclic or cyclic functions and
  edge-initialized block parameters in acyclic or cyclic functions; admitted
  uses now include body and terminator operands plus register-transport
  arguments on outgoing successor edges.
  The owning paths are `selected-instructions-to-register-homes/src/assignment/runtime_spill/`
  and `selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`.
  Complete broader CFG/type and fixed-use recovery, composition with selected
  recovery rules, physical slot reuse/coloring, and independent validation.
  Existing logical spill plans grant no frame, unwind, instruction, or
  publication authority.
  Extend final spill-inclusive frame demand into stack provisioning under the
  [compiler-owned stack contract](wiki/spec/resources/storage.md#compiler-owned-stack-accesses).
  Keep `WRITE-ONLY-BORROW`'s source-backed three-call regression as the hosted
  execution control, including its installed-image demand replay and stale-frame
  rejection. Reuse the existing final-local-frame demand path, not a parallel
  spill-byte estimate.
  Acceptance: slot reuse is not double-counted; changed allocation/frame
  realization invalidates stale demand; insufficient supply rejects before
  execution; and generated loads/stores independently replay their physical
  geometry and value lineage without adding source crash routes. A byte ceiling
  alone must not stand in for valid stack backing. Target-required probing and
  setup remain frame/provisioning work, not an owner-blocked language choice.

- **ALLOCATION-REFINEMENT.** Complete coalescing, live-range splitting,
  fixed/precolored intervals, and rematerialization cost decisions while
  preserving exact register-unit aliases, liveness, and target custody.
  Landed: runtime-spill recovery orders rematerialization before private
  storage — a pressured victim defined by one pure MaterializeI64
  regenerates a fresh materialization at each admitted flexible use, creating
  no reload interval or stack slot, independently replayed through the
  post-allocation manifest ledger (codec 9) on all four targets
  (`runtime_rematerialization_pressure::loop_carried_rematerialization_replays_through_callable_publication`).
  Landed: home-assignment coalescing ranks legal candidates by the guaranteed
  coalesces a still-unassigned constrained neighbor would lose, after
  assigned-partner edges and partner votes, mirrored in independent replay and
  the scan reference
  (`copy_affinity_avoids_stealing_a_constrained_neighbors_guaranteed_home`).
  Landed: runtime-spill recovery admits fixed-view body instruction uses —
  a pressured victim feeding an ABI-pinned call operand gets its own
  address/load pair immediately before the consumer while the operand
  keeps its fixed view, pinning the fresh reload to that physical unit
  for exactly the load-to-use window on all four targets
  (`fixed_view_instruction_uses_pin_their_reload_at_the_call_operand`).
  Remaining: coalescing across split points, live-range splitting, and
  sequencing the fixed/precolored interval stages on the default
  recovery path.

- **FRAME-LAYOUT.** Extend exact nonzero-frame realization beyond the landed
  CFG families: red-zone policy, probing, unwind
  information, stable-address loans, and dynamic-allocation constraints.
  General calls need target-owned frame, callee-save, link-register, and
  call-site alignment plans. Witnessed: the ordinary three-block/two-return
  fixture and the cyclic loop-carried runtime-spill fixture (six emitted
  blocks, backward branch, nonzero local spill storage) both replay through
  frame application, relocation-free object construction, and validated
  ordinary callable publication on x86-64 and AArch64
  (`runtime_spill_pressure::loop_carried_spill_frame_replays_private_accesses_through_callable_publication`).
  A scalar caller passing one argument past each target's register capacity
  now replays its exact outgoing-ABI-area write and the callee's exact
  incoming-activation read against the validated frame geometry through the
  same publication boundary on all four targets
  (`register_arity::stack_argument_calls_replay_frame_accesses_through_callable_publication`).
  Probing is landed: a caller whose outgoing ABI area exceeds one
  stack-commit granule commits its frame through an exact per-granule
  touch roster recorded in the validated layout and replays through
  ordinary callable publication on all four targets
  (`stack_probe_commit::wide_outgoing_area_commits_through_exact_probe_roster_and_publication`).
  x86-64 emits move-and-touch chunks; AAPCS64 emits a
  shifted-then-unshifted `sub sp` pair per chunk followed by an
  `ldr xzr, [sp]` touch of each newly entered page, and `FrameAddress`
  forms every displacement a committable frame resolves through the same
  add pair (both Darwin's 16 KiB unprobed granule and Linux's 4 KiB
  probed granule publish). The red-zone policy is landed: a leaf whose
  fixed-register divide pressure recovers through runtime spill keeps its
  whole addressed extent resident below the unadjusted entry stack pointer
  on System V AMD64 — empty prologue and epilogue spans, signed
  below-RSP displacements replayed row by row against the validated
  geometry — while the same program stays an ordinary committed frame on
  every other admitted ABI, a suppressed or invented resident extent
  replays false, and the artifact still publishes as an ordinary callable
  on all four targets
  (`red_zone_resident_frame::resident_leaf_spill_frame_publishes_through_ordinary_callable_entry`).
  Acceptance: every admitted frame policy
  replays its exact physical accesses through callable publication;
  requirements artifacts remain non-authoritative until that replay
  succeeds. Remaining: unwind information,
  stable-address loans, and dynamic-allocation constraints; general
  calls still need target-owned frame, callee-save, link-register, and
  call-site alignment plans beyond the landed callee-save frames.

- **GENERAL-CALL-CLOBBERS.** Extend live-across-call allocation and clobber
  validation from the landed attached-Unit fork/join slice through general
  scalar and structural calls on each ABI. Witnessed: an ordinary
  scalar-returning caller keeping a parameter and earlier call results live
  across later calls reports and independently replays callee-saved
  requirements on `linux_x64` and `linux_arm64`
  (`tests/native-differential/.../fixtures/scalar_call_preserving.rs`,
  `.../register_allocation/callee_saved_requirements/`). The same callers now
  report, store, and replay convention-specific preservation on all five
  native targets: System-V, Microsoft x64 (Windows and UEFI), AAPCS64, and
  Darwin AAPCS64, including roster distinctions (Microsoft x64 adds rdi/rsi
  and xmm6-15; Darwin moves x18 from caller-saved to fixed), clobber-candidate
  exclusion at call points, exact callee-save slot geometry, and per-target
  frame-layout corruption replay
  (`.../register_allocation/callee_saved_requirements/`,
  `.../register_allocation/callee_save_storage/`,
  `.../register_allocation/runtime_scalar_call_chain/`). A caller passing a
  borrowed structural argument through three scalar-returning call sites —
  extent pointer live across every call, scalar parameter live across the
  first call, first result live across the second — now reaches the same
  chain: clobber-candidate exclusion and callee-saved homes on all five
  targets, legalized-plan and call-template corruption replay on all five
  targets, exact frame-layout and protocol-encoding replay, and fixed-frame
  object-artifact publication with custody rejection on both Linux ISAs
  (`tests/native-differential/.../fixtures/structural_call_preserving.rs`,
  `.../register_allocation/structural_call_chain/`). The bounded
  reload-homes lane now witnesses a reload interval that survives an
  intervening call: under an explicit physical-view allowlist a `CallUnit`'s
  clobber set reduces the spill victim's common candidates to the single
  callee-saved view (rbx on x86-64, x19 on AArch64) on all five native
  targets, and independent replay rejects a caller-saved home, cleared
  candidate or coexisting-home rosters, a shrunken interval, and root or
  usage mismatches
  (`tests/native-differential/.../fixtures/call_spanning_reload.rs`,
  `.../register_allocation/reload_value_homes.rs`). Remaining: the
  executable runtime-spill rewrite still realizes each use as a private
  reload, so no produced interval needs to survive an intervening call
  (`selected-instructions-to-selected-instructions/src/rewrites/runtime_spill.rs`
  excludes them), and the sequenced allocation route does not yet reach the
  bounded lane's call-crossing capability.

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the landed symbolic instruction-pair
  descriptors (`selected_lowering/literal_fold/pair_rule.rs`) to physical
  register units, effects, traps, memory, stack, and control flow without
  replacing the independent validator. Landed: `PairResultDisposition`
  declares whether the rewritten instruction delivers its result through a
  scalar `Def` operand or implicit physical-unit condition-state
  definitions, and the producer admits constraint-row and consumer operand
  shapes through it (crate `nextest`: 157 pass; a `CompareI64` consumer
  carrying a scalar-result shape now rejects as `ConsumerMismatch` instead
  of indexing past the row). `PairUnitEffects` covers implicit-unit uses,
  clobbers, and operand unit bindings; `PairMachineEffects` covers the
  memory, trap, stack, control-flow, barrier, call, and cleanup surface;
  `PairOperandShape` now carries `BinaryLeftLiteral` — the commuted
  exact-add grammar folding the literal at operand 0 through the same
  `ExactAddI64Immediate` row — so the exact-add selection declares one pair
  per operand position and the recorded action binds the surviving register
  rather than a fixed operand (226 crate lib tests pass, including firing
  on both targets plus decision-field corruption and wrong-position
  negatives; the replay re-derives the grammar from the consumer kind and
  recorded operand position alone).
  `PairMachineEffects::IndexedPointerReadFold { index_operand }` declares
  the first non-isolated relationship — an indexed pointer read folded to
  an immediate-offset read — and `LOAD8_INDEXED_U12` folds a
  `MaterializeI64` index at operand 1 of `Load8Indexed` into `Load8`'s
  byte offset under `LiteralFoldPolicy::LOAD8_INDEXED_V1`, bounded at 4095
  by the aarch64 displacement field (crate `nextest`: 235 pass, including
  firing on both Linux targets, the unencodable-4096 boundary,
  decision-field substitution, and wrong-policy negatives; the validator
  restates the relationship independently as `indexed_read_fold_admission`
  instead of reusing the pair descriptor). Remaining: further unit roles
  and the other non-isolated relationships — trap-, stack-, and
  control-flow-carrying forms still have no descriptor variant.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Add copy removal, redundant extension
  removal, address folding, compare/test selection, and scheduling only where
  each transformation is independently verifiable. Existing narrow same-view
  and compare-adjacent cases do not imply general authority. Landed:
  `rewrites/redundant_extension` rewrites an extension whose input's unique
  producer already guarantees the normalized bits to `CopyI64`, and
  `rewrites/literal_compare` rewrites a `CompareI64` whose right operand's
  unique producer is a `MaterializeI64` inside the shared twelve-bit
  unsigned immediate bound to `CompareI64Immediate` — or `CompareI64Zero`
  for a literal of zero — keeping the compare's identity, position, and
  published flag surface while retaining the materialization for other
  readers — and `rewrites/copy_removal` removes a `CopyI64` whose
  destination's only mentions are plain `Use` operands later in the same
  block, rebinding each to the source and dropping the destination roster
  row, admitting a source redefinition on the last use's own instruction
  but none inside the open interval, each under replayed
  restore-by-content validation (crate `nextest`: 254 pass). Remaining:
  address folding, scheduling, left-operand literal folding (immediate
  forms fix the literal as the subtrahend), and producers whose contracts
  do not fix the high bits (`ZeroExtendU32` output, packed loads, FP bit
  transfers).

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Add borrow-aware load forwarding, dead-store
  elimination, and mutation motion on the selected CFG, each decided from
  the validated `memory_accesses` roster under place exclusivity and
  replayed by restore-by-content validation. Landed:
  `rewrites/load_forwarding` rewrites a `Load64`/`Load32`/`Load16`/`Load8`
  whose same-block writer is a `Store` with the read's exact same-width
  `WritePlace` row at the identical byte offset — `CopyI64` of the stored
  register at full width, the matching `ZeroExtend` at sub-word width,
  since a same-width store-then-load round-trips the register's low bits
  in the target's own byte order — and `rewrites/dead_store` removes a
  `Store` whose dead range a later `Store` covering the same row
  rewrites unobserved — the first access on the dead place decides,
  boundary settlements inside the dead interval reject while later
  positions shift one ordinal. Dead-store search also walks forward
  across edges when every outgoing edge of a crossed block names one
  block — each path forward then reaches the covering store before any
  observer — checking each crossed terminator's roster rows and each
  crossed edge's transports: joins at crossed blocks are harmless
  because coverage looks forward, while forks to distinct blocks,
  returns and hosted exits, re-entered blocks, and edge transports
  writing or retiring the dead place's storage end the walk unproven.
  `rewrites/store_motion` sinks a place `Store` carrying exactly one
  `WritePlace` row to just before the first position that must stay
  ordered after it — an interfering roster row on the moved place, a
  call or hosted effect, a carried pointer or value redefinition, an
  unaccounted memory-capable instruction, or a boundary settlement —
  and keeps walking forward across a terminator when every outgoing
  edge names one block that sees the crossed block as its only
  predecessor, checking each crossed terminator's roster rows and each
  edge's transports: forks to distinct blocks, joins into the
  successor, successors that can reach back to the walked chain, and
  transports redefining the carried registers or writing the moved
  place's storage land the store at the last proven position rather
  than rejecting. The roster names the store by instruction identity,
  so it stays unchanged; boundary settlements at or after a
  crossed-block landing shift one ordinal. Load forwarding also walks
  back across edges when the load's block has exactly one predecessor
  block — every
  path to the load then carries that block's writer — checking each
  crossed terminator's roster rows and each crossed edge's transports:
  edge-defined register parameters colliding with the carried value or
  the load's result reject, as do structural destinations, case custody
  slots, and custody discards touching the forwarded place's storage,
  while joins, unreachable blocks, self-loops, and the function entry
  end the walk unproven (crate `nextest`: 282 pass). Remaining: killers
  beyond the exact `Store`/`WritePlace` pair; forwarding still stops at
  joins.
- **REPRESENTATION-SPECIALIZATION.** Add field/variant relevance and
  invariant-window specialization.
- **CLEANUP-PRUNING.** Add cleanup and transition reachability pruning without
  losing affine/linear custody.
- **STATE-SPECIALIZATION.** Add state-argument/result specialization with exact
  edge provenance.
- **INTERPROCEDURAL-SUMMARIES.** Add service/call summaries and proof-bound
  inlining.
- **PROOF-DIRECTED-LOOPS.** Add loop-bound reasoning, induction
  simplification, and vectorization with exact lane semantics.

## Verification and rollout

- **PER-RULE-COVERAGE.** Finish positive, negative, boundary, disabled, budget,
  determinism, fixed-point/idempotence, and corruption coverage for every exact
  rule. Do not call repeated reconstruction idempotence when the published
  artifact is not a legal second input.

- **TARGET-MATRICES.** Complete supported target/OS allocator, encoding,
  unwind, object, and callable matrices. Existing selected-lowering and
  post-allocation matrices do not claim physical spill insertion, final frame
  layout, or unwind completion. The callable leg now declares its second
  boundary mechanism: `CallingPolicy::native_syscall_for_target` maps the
  Linux x86-64 and arm64 pairs to their direct-syscall policies, answers
  none for the Windows, UEFI, and Darwin C-called boundaries, and fails
  closed on undeclared pairs; provider-planning's syscall binding arm now
  selects from this matrix rather than a private (format, architecture)
  match (calling-conventions `nextest`: 66 pass on Linux x86-64, including
  per-target row pinning, undeclared-pair, and wrong-architecture drift
  negatives). The encoding leg is now declared on both ISAs: isa-aarch64
  `selected_keys` resolves one `Aarch64SelectedAbi` per supported
  (architecture, object-format) pair — (Aarch64, Elf) AAPCS64 and (Aarch64,
  MachO) Darwin — so call, aggregate, and float-return rosters follow the
  declared family rather than a non-ELF fallback, and undeclared (Aarch64,
  Coff) fails closed with `UnsupportedTargetAbi` before any row is selected
  (`cargo test -p isa-aarch64`: 102 pass on Linux x86-64, including declared
  pair pinning, the undeclared-Coff arm, and exact-target hosted rows).
  Remaining: allocator, unwind, and object legs.
  Windows and macOS runs were unavailable on this host.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
