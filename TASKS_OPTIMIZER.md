# Optimizer tasks

This is the optimizer execution board, not its history. Architecture and
rationale live in
[optimizer implementation](omega-rust/optimization.md),
and landed milestones live in Git. Remove work from this file when its
acceptance condition passes.

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

- **PSI-PRE-TERMINAL-OPTIMIZATION.** Complete selected rewrites and independent
  checks in `lowered-psi-to-lowered-psi` before `lowered-psi-to-terminal-psi`:
  control-flow cleanup, SCCP, copy propagation, GVN, dead pure scalar elimination
  and proof-check elision. Proof-bearing closures need proof-context transport.
  Acceptance: exact `build.omg` opt-ins execute before immutable Terminal
  publication, preserving proof, ownership, effects, qualifications and execution
  evidence. Standalone Psi and separately authorized resumed lowering need no
  original frontend state or hidden consumer-side Psi optimization. Landed:
  dead total scalar elimination also removes unused scalar block parameters and
  their edge arguments in proof-free, unranked machines; copy propagation
  collapses scalar block parameters bound to the same resolved value on every
  inventoried incoming edge, substituting the resolved source and dropping the
  matching edge-argument positions while retaining every value a proposition,
  projection, suspension frontier, ranking row, or recorded source-call join
  names; SCCP, GVN, control-flow cleanup and proof-check elision remain open.

## Product pruning and rollout

- **CHECKED-TREE-PRODUCT-PRUNING.** After all authored code has been checked,
  add an exact opt-in checked-tree phase for whole-product/root selection and
  unreachable declaration pruning. Retain selected-product identity and all
  ownership, proof, effect, boundary, and diagnostic obligations. Acceptance:
  pruning cannot hide an invalid authored declaration; shared frontend work is
  reusable before multi-target fan-out; each target/root-selected Psi product is
  independently reproducible.

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
  `PackageInstance` construction.

- **GENERATED-DIFFERENTIALS.** Extend same-artifact interpreter/native
  differential testing beyond the landed exact-integer lane to float, trap,
  atomic, placed-memory, cleanup, and transition behavior. Acceptance: fixed
  seeded corpora replay deterministically and native observations equal the
  reference interpreter under the same observation profile.

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
  Remaining: dynamic tables.

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
  relocation is not general LICM authority.

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
  coverage on both Linux targets. Note: selected-lowering selections still
  reject at the common physical-staging gate
  (`UnconsumedPostTerminalPhase(SelectedLowering)`, retired downstream
  dispatch from `267f5eb205`), identical to the add/subtract families.
  Remaining: further families (address-mode folding, extension elimination,
  constant materialization) one exact named family at a time.

- **SELECTED-ABI-VALIDATION.** Validate ABI operands, calls, clobbers, effects,
  traps, provenance, cleanup, and logical fuel across every selected rule.

## Register allocation and frames

- **SPILL-REALIZATION.** Extend executable spill recovery beyond dominating
  nonaddress instruction results in acyclic or cyclic functions and
  edge-initialized block parameters in acyclic functions.
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

- **FRAME-LAYOUT.** Extend exact nonzero-frame realization beyond the landed
  CFG families: stack-argument storage, red-zone policy, probing, unwind
  information, stable-address loans, and dynamic-allocation constraints.
  General calls need target-owned frame, callee-save, link-register, and
  call-site alignment plans. Witnessed: the ordinary three-block/two-return
  fixture and the cyclic loop-carried runtime-spill fixture (six emitted
  blocks, backward branch, nonzero local spill storage) both replay through
  frame application, relocation-free object construction, and validated
  ordinary callable publication on x86-64 and AArch64
  (`runtime_spill_pressure::loop_carried_spill_frame_replays_private_accesses_through_callable_publication`).
  Acceptance: every admitted frame policy replays its exact physical accesses
  through callable publication; requirements artifacts remain
  non-authoritative until that replay succeeds.

- **GENERAL-CALL-CLOBBERS.** Extend live-across-call allocation and clobber
  validation from the landed attached-Unit fork/join slice through general
  scalar and structural calls on each ABI. Witnessed: an ordinary
  scalar-returning caller keeping a parameter and earlier call results live
  across later calls reports and independently replays callee-saved
  requirements on `linux_x64` and `linux_arm64`
  (`tests/native-differential/.../fixtures/scalar_call_preserving.rs`,
  `.../register_allocation/callee_saved_requirements/`). Remaining:
  structural-argument calls, the Windows x64, UEFI, and macOS preservation
  conventions, and reload intervals that survive an intervening call
  (`selected-instructions-to-selected-instructions/src/rewrites/runtime_spill.rs`
  excludes them).

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the landed symbolic instruction-pair
  descriptors (`selected_lowering/literal_fold/pair_rule.rs`: producer,
  consumer, and rewritten `MachineSemanticKind` plus one immediate bound,
  read by the producer only) to physical register units, effects, traps,
  memory, stack, and control flow without replacing the independent
  validator.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Add copy removal, redundant extension
  removal, address folding, compare/test selection, and scheduling only where
  each transformation is independently verifiable. Existing narrow same-view
  and compare-adjacent cases do not imply general authority.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Add borrow-aware load forwarding, dead-store
  elimination, and mutation motion.
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
  layout, or unwind completion.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
