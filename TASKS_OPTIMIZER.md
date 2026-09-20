# Optimizer tasks

This is the optimizer execution board, not its history. Architecture and
rationale live in
[optimizer implementation](omega-rust/optimization.md),
and landed milestones live in Git. Remove work from this file when its
acceptance condition passes. An open item's evidence states the current
frontier once: recording a newer observation deletes the dated paragraph it
supersedes rather than appending beside it. Tag an added item's provenance on its first line:
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
  Every pipeline crate is already a literal `X-to-Y` or `X-to-X` stage and the
  folders match the [connected route](omega-rust/pipeline.md#connected-program-route);
  `tests/architecture/representation_ownership.rs` pins the `X-to-X` phase
  directories and keeps native coordination and target setup out of the stage
  list. That does not establish the acceptance below: validated owners with
  public entrances still sit beside the route without being called from it.

  Remaining work:

  - `selected-instructions-to-selected-instructions/src/rewrites/` holds about
    40 rewrite modules (`copy_removal`, `dead_store`, `load_forwarding`,
    `store_motion`, `address_fold`, `literal_compare`, `redundant_extension`,
    the relocation and interchange families) whose entrances, such as
    `remove_selected_copy` and `eliminate_selected_dead_store`, are called only
    from their own tests. `optimize_selected_instructions` runs
    `run_selected_lowering_optimizations` and nothing else, and none of these
    rewrites has an exact name in
    [rules.md](omega-rust/omega/representations/optimization-core/rules.md).
    Give each retained rewrite a catalog entry executed by the stage entrance,
    or delete it. Their behavior stays with `EXACT-MACHINE-SIMPLIFICATIONS`,
    `DECLARATIVE-PEEPHOLES` and `ALIAS-AWARE-MEMORY`.
  - `selected-instructions-to-register-homes/src/unsequenced_spill_stages/`
    holds 18 spill families, each with public plan and receipt records, that
    `stage_register_allocation` never calls. `SPILL-REALIZATION` owns
    sequencing the ones it needs; delete or merge the ones the executable
    `assignment/runtime_spill` route has superseded.
  - `native-realization/src/optimized_semantic_wrapper_{encoding,object}/`
    keeps a staged record, codec and validator inside a coordinator crate.
    `select_optimized_program_storage_semantic_wrapper_encoding` and
    `stage_validated_optimized_program_storage_semantic_wrapper_object` have no
    caller outside their own tests. Move the live part to its backend or
    representation owner, or delete it.
  - Audit the remaining stage and coordinator crates the same way: a public
    stage entrance that no coordinator or successor stage calls is an orphan
    output.

  Acceptance: folders expose the connected program sequence, no competing
  entrances or orphan outputs remain, and coordinators only sequence typed
  stages. Renaming a helper or adding a wrapper is not completion.

  Flag: three owners carry validated, replayed and mutation-tested machinery
  that no executable route reaches (about 40 selected rewrites, 18 spill
  families, the ProgramStorage wrapper object). Each new slice adds tests and
  board text for code the compiler never runs. The general mechanism is the
  one [optimization.md](omega-rust/optimization.md#catalogs-and-independent-replay)
  already names: one ordered catalog per owning stage, executed by the stage
  entrance.

- **REPRESENTATION-OWNERSHIP.** Finish
  `omega-rust/{omega,psi}/representations/` under
  [native representation ownership](omega-rust/omega/representations/README.md):
  one named program-root file beside `lib.rs`, concept-owned subdirectories,
  and current data independent of producer history.
  `tests/architecture/representation_ownership.rs` enforces the named root
  for all ten Psi representations and the Omega program representations
  (abstract, boundary, target, legalized and selected operations, register
  homes, physical instructions, machine code, representation selections,
  optimization unit) and rejects `StagedOptimized` ancestry inside them. The
  shared-vocabulary table pins the root sets of optimization-core,
  register-model, task-plans, effects, calling-conventions,
  function-identity and installation-evidence. `representations/target` is
  the one Omega representation crate the guard does not yet name, and
  consumers in the selected and allocation stages still reach current data
  through producer history.

  Remaining work:

  - Replace stage-ancestry walks with direct reads of the current program.
    `selected-instructions-to-selected-instructions/src/selected_optimization.rs`
    obtains its selections through
    `ranges.liveness_stage().selected_stage().optimized_target().optimized()`;
    the same accessor chains occur on about 100 non-test lines of
    `selected-instructions-to-register-homes` and about 60 of
    `selected-instructions-to-selected-instructions`. Keep the retained inputs
    as replay evidence only.
  - `representations/optimization-unit` settled as a named representation at
    `11eaa140cb`: `optimization_unit.rs` is the one root beside `lib.rs` and
    every concept area — including the `construction/` projection entrance —
    nests under `optimization_unit/`. The projection stays with the
    representation because validation replay and test fixtures outside the
    producer stage consume `reconstruct_psi_optimization_unit_seed` directly.
  - Move durable codecs out of transforms and coordinators with their
    consuming stage changes: `post_allocation_manifest/codec` and
    `rewrites/allocation_recovery/fixed_view_copy/codec` in the two selected
    stages, and `optimized_semantic_wrapper_object/codec` in
    `native-realization` (see `PIPELINE-OWNER-CONSOLIDATION` for whether that
    owner survives).
  - `representations/target` is the last unnamed crate: decide whether it is
    shared vocabulary (exempt under
    [pipeline.md](omega-rust/pipeline.md#placement-and-semantic-ownership)) or
    a program needing one root, and extend the guard's table to match.

  Acceptance: current programs outlive their producers; ordinary consumers
  read current data directly; historical inputs remain separate replay
  evidence; the architecture guard names every program representation.

## Product pruning and rollout

- **WORKSPACE-ROLLOUT.** Keep every rule explicit opt-in and `Experimental` in
  the [exact-rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  until the frozen-tree command `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test
  --workspace --no-fail-fast` passes. Do not replace it with `--all-targets`,
  which omits doctests. The command cannot pass today: `AGENTS.md` records a
  full `canary_suite` run as red, and
  [known baseline failures](wiki/drafts/known_baseline_failures.md) lists open
  library and native-differential failures. The first staged record,
  [promotions/ControlFlowCleanup.md](omega-rust/omega/representations/optimization-core/promotions/ControlFlowCleanup.md),
  records that rule's coverage state with `Approved status`, owner approval,
  and measurement evidence still `PENDING`; `omega-architecture-test`'s
  `exact_rule_rollout_is_complete_and_promotion_gated` keeps the inventory and
  every staged record in step and resolves each record's backticked
  `path`/`path::subject` evidence citations against the checkout, so a
  promotion leg passes only on artifacts that exist and name their subjects.
  Acceptance: the command passes from a clean checkout and every promoted
  exact rule has the evidence the
  [promotion contract](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
  requires.

## Validation, translation, and publication

- **TRANSLATION-VALIDATION.** Complete independent source-to-target replay for
  admitted operations and transfers on the common graph. Every surviving
  executable boundary occurrence binds exactly one physical child under
  [closed application and physical occurrence](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence);
  `native-artifact/src/physical/derivation/` derives those children, and
  selected-lowering operations, both occurrence roles, and the exemption for
  verified-eliminated occurrences already replay through it with missing,
  duplicate, stale, substituted, padded and role-swapped children rejecting.
  Coverage is attributed per occurrence rather than all-or-nothing: a
  surviving occurrence whose realization admits no span arm yields a
  `Blocked` `NativePhysicalEvidenceGap` naming its exact subject, and
  `require_native_physical_evidence` surfaces it as
  `NativePhysicalEvidenceBlocked` instead of erasing the artifact's
  evidence unattributed.

  Remaining work:

  - Operator applications. `physical/operator_applications.rs` spans
    `NongenericCheckedBody` and `SpecializedCheckedBody` — a direct call
    span, or a fragment call under fragment publication; both arms cover
    all five static call kinds (`Call`, `CallUnit`, `CallStructural`,
    `CallStructuralScalar`, `CallStructuralWithScalarArguments`) — and
    `ExactCompilerIntrinsic`, as an FMA span plus an IEEE float-compare
    fragment. `CallDynamic*` kinds carry descriptor or parameter ordinals
    rather than a static callee, and other intrinsic kinds have no span
    arm, so those occurrences remain named gap subjects. Regression:
    `physical_child_replay::structural_result_operator_occurrence_replays_one_exact_physical_child`
    (Linux x86-64) drives a structural-result boundary operator through
    emission, exact-child binding, and every mutation-class rejection.
  - Boundary settlements. `derivation/evidence.rs` joins each installed
    settlement's closed `(CompilerBuiltinExecution, BoundaryRealization)`
    pair against `HOSTED_BUILTIN_SETTLEMENTS` in `derivation/children.rs`
    (`HostedExitProcessI32`, `HostedWriteByteI32`, `HostedReadByte`), where
    one row declares the builtin's supported targets, admitted
    scalar-argument forms, and result custody for the shared
    `derive_hosted_builtin_child` span join — then the admitted-provider
    settlement, then the normalized foreign call. A fourth hosted builtin
    is one catalog row, not a new derivation. Any other builtin, and any
    occurrence carrying neither an installed settlement nor a foreign call,
    yields no evidence.
  - Privileged port effects. Every retained effect must be consumed by an
    exact `MetadataOnlyPort` settlement join; one unowned effect drops the
    artifact's evidence.
  - General calls wait on `FRAME-LAYOUT`, itself blocked on a contract for
    runtime-sized activation storage.

  Acceptance: a program whose occurrences include a realization outside those
  arms publishes complete physical evidence binding each surviving occurrence
  to one child with nonempty machine, object and final-image spans, and replay
  rejects missing, duplicate, stale, substituted, padded and role-swapped
  children. An artifact that cannot span an occurrence names that occurrence
  instead of publishing with no evidence at all. These are native compiler
  guarantees, independent of package locks or `PackageInstance` construction.

  Scalar expression-family planners, Unit and structural whole-function
  templates, and their catalogs and compatibility fixtures are retired. Do not
  restore them to recover arithmetic, crash, cleanup or borrowed-call coverage;
  extend ordinary graph operations and their receiving checks instead.

- **CUSTODY-MUTATION-COVERAGE.** Complete authenticated one-field mutation
  tests for every remaining manifest, receipt, codec, and artifact-custody
  family, as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires. `*_rejects_every_one_field_substitution` and field-substitution
  legs now cover the image-emission installation records, executable
  installation and its wire container, component publication and description,
  external-root admission, the compiler's object-artifact, object-container,
  realization, text-section, callable-entry and fragment-emission custody, the
  production compilation manifest, the post-allocation machine staged plan,
  build and package records, the Terminal codec sections, and every
  `Staged*CustodyReceipt` family in `omega-rust/omega/pipeline/` — selection,
  liveness, live ranges, allocation legality, selected reanalysis,
  fixed-precolored segment homes, fixed-view copies, the literal-fold
  sequence, the selected-lowering run, baseline/post-copy/post-literal-fold/
  post-selected-lowering register homes, active-resident rematerialization
  and its pressure receipt — plus the machine plan receipt. Each leg mutates
  one field through a declared `*FieldForTest` inventory, recomputes the containing identity
  honestly where the record carries one, and requires the family's named
  independent checker to reject; each hook names its checker and lists the
  fields closed by single-variant vocabularies instead of leaving them
  unlisted.

  Acceptance: each representable field of a family changes independently, its
  containing identity recomputes honestly, and independent replay still rejects
  the substitution; a field that cannot be represented is rejected at canonical
  encoding and named as such. A new record family lands with its matrix rather
  than acquiring one later.

  Flag: the matrices are hand-written, one per record shape.
  `image-emission/tests/artifacts/installation_function_nested_custody.rs` alone
  holds 18 of them in 6,942 lines, and six such files run 15,157 lines
  together; only the wrapper-object matrix under
  `native-realization/src/optimized_semantic_wrapper_object/tests/` is factored
  into a reusable shape, and the pipeline families declare their inventories
  through per-family `*FieldForTest` enums and `corrupt_custody_for_test`
  hooks without a shared driver. Nothing fails when a family has no matrix.
  The general mechanism is one substitution harness driven by each record's
  canonical field inventory plus a per-family honest-recomputation hook, so
  that a new family declares its fields instead of adding another
  several-hundred-line test, and a family with no entry fails a repository
  gate.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Carry ordinary cyclic Terminal Psi through the
  post-Terminal stages to native publication. Natural-ranked and unranked
  modules already take the ordinary verification and abstract-lowering route
  ([ranked native admission](omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#ranked-native-admission)),
  the optimizer authenticates their components and freezes the complete cyclic
  function under
  [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay),
  and admission rejects the retired unsigned-countdown custody outright instead
  of falling back. Artifact admission alone establishes no downstream
  optimization, target lowering, or publication support for those cycles. The
  item of the same name in `TASKS.md` owns the Psi half — Terminal production
  of cyclic machines, the verifier's cyclic shape allowlist, and the
  `print_squares` and Console-writer customers. This item starts at admitted
  Terminal input and owns the receiving graph, native selection, and replay.

  Remaining: implement
  [ranked callees on projected receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers),
  beyond the current whole-entry-only admission. Composed argument references,
  call and return, cleanup, callee measure checking, and composed resource
  evidence are all missing; a graph representation establishes none of them,
  and unsupported transfers reject until their ordinary operation and proof
  joins exist. Extend the common graph and ordinary ranking evidence rather
  than restoring a second native route.

  Acceptance: an ordinary caller borrows a nested field, its ranked callee
  preserves that referent across backedges, and the caller observes writes
  after return. Conflicting parent access and missing or invalid callee ranking
  reject. Argument identity, call and return, cleanup, and composed resource
  bounds validate through native replay and execution on both Linux
  architectures. This needs no new receiver syntax; broader changing-reference
  transfers still require loan/alias and ranking substitution work, and
  widening the parameter count alone closes none of them.

- **GENERAL-LICM.** Implement motion only through transformations that
  invalidate and reconstruct component, loop-carried custody, ranking,
  provenance, effect, and fuel evidence. The dedicated countdown zero/one
  relocation is not general LICM authority. The shared-source preheader
  boundary in `abstract-operations-to-abstract-operations`
  (`src/ranked_rewrites/loop_invariant_scalar_motion/`) relocates scalar
  constant leaves and side-effect-free scalar computations, place observations,
  byte-sequence reads, subslices and literals, primitive locals, records,
  scalar arrays, scalar cases, and unit, scalar-signature, structural-scalar
  and affine structural calls, each behind one `admissible_invariant_*`
  predicate in `src/validation/mod.rs`. Invariant discovery resolves member
  scalar and structural parameters transitively across component-internal edges
  to the representative every reaching edge agrees on; the insertion preheader
  is the one block every authenticated entry edge departs; and the freeze fence
  re-derives the non-speculative gate from the authenticated topology rather
  than trusting the proposal.

  Remaining work:

  - Other non-scalar families. `EstablishTrivialAffineLocal` has no admitted
    relocation evidence and falls to the scalar-result fallthrough in the
    verifier's `unranked_cycles::eligible`. `CallStructural` relocates only in
    its affine claim-free form bound to the same block's `StructuralCase` or
    `ReturnStructural` terminator — an unrestricted place result would leave
    later traversals dispatching a disposed place — and its structural
    arguments admit only the non-owned borrows under the same
    place-custody bound and root-landing rule `CallUnit` and
    `CallStructuralScalar` replay, run with the relocating confined results
    tolerated: an `Owned` argument would move the caller's place into the
    callee, custody this boundary cannot re-express. The remaining
    establishments still need an admitted cyclic source shape — scalar-graph
    arrays only emit as call arguments — so that work starts upstream in psi.
    Synthetic fixtures cannot bypass the fence: optimizer admission replays
    `verify_module_for_optimization`.
  - Profitability has no bounded leg. Every operation and terminator costs one
    fuel unit, so no zero-cost speculation exists beyond the constant-leaf
    exemption.
  - Motion past the shared-source preheader needs new blocks or run
    duplication, which the frozen block roster and the unique-occurrence freeze
    reject.

  Region custody constrains every remaining motion family: the counted-loop
  `LoopRegion` is projected from validated Terminal-SCC custody, never from a
  private loop forest or a second edge/reachability walk inside the countdown
  leaf. Reducibility under the certified header rests on the custody's unique
  entry edge landing on that header plus the verifier's all-blocks-reachable
  control graph — a component holding the machine entry block has no entry
  edge at all, because any non-member reaching into it would join its cycle.
  The independent reconstruction in
  `src/validation/context/ranked_cycles/ordinary.rs` rebuilds components from
  the current optimizer body and requires them to equal the verifier's
  Terminal surface exactly. Keep both halves; do not reintroduce a loop-forest
  producer to recover a region.

  Acceptance: each added family relocates under an admission that freeze replay
  re-derives independently from the transformed graph, with ownership-frontier
  membership invalidated and rebuilt rather than trusted, and rejects forged
  operands, retained member-internal discards, missing exit disposals, and
  stale frontier catalogs. Component, loop-carried custody, ranking,
  provenance, effect and fuel evidence all reconstruct after the move.

## Register allocation and frames

- **SPILL-REALIZATION.** Finish executable pressure recovery and join its
  frames to stack provisioning.
  [Register allocation](omega-rust/omega/pipeline/selected-instructions-to-register-homes/README.md)
  chooses victims in `src/assignment/runtime_spill/`; the rewrite owner,
  `selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`,
  inserts and independently replays the private stores and reload pairs. That
  route already spills instruction results, block parameters and entry-bound
  registers in acyclic and cyclic functions — including IEEE-scalar victims
  whose class the frame rows cannot carry, by wrapping every store and
  reload in the target's inert `Float*ToBits`/`BitsToFloat*` pair — serves
  body, terminator, edge-transport and stored-snapshot uses; keeps an ABI
  pin on its own reload pair; carries a reload across a call onto a
  surviving view; and composes after fixed-view copies and active-resident
  rematerialization. It does not establish the cases below.

  Remaining work:

  - Victim and use admission (`rewrites/runtime_spill/admission.rs`). A
    foreign-class IEEE scalar reaches its slot through the frame rows'
    shared carrier class: stores prepend `Float*ToBits`, reloads append
    `BitsToFloat*`, and a missing or impure conversion row keeps the victim
    a candidate-local rejection. Address-defined victims
    (`FrameAddress`, `AddressOffset`, `ByteViewAddress`) admit at
    `3ab7564c05`; redefining `Def` operands admit at `58c5089231`; and tied
    Def+use and `UseDef` operands admit at `f8d6064244`. Still rejected: a
    foreign-class victim without that transport pair (vector-class values,
    non-IEEE scalars); an early-clobber write tied to a victim use; an
    entry-bound victim when an edge targets the entry block; and a
    multi-chunk stored snapshot whose chunk loads are pinned or separated by
    a unit-writing instruction. Recovery then tries the next roster
    candidate and fails when the roster is exhausted.
  - Slot assignment. Every victim declares a private eight-byte
    `LocalStorageSlotId::Spill` slot. `runtime_spill/slot.rs` shares an
    existing slot only for the zero-offset `Store64` or
    `FrameAddress`-plus-`Load64` idiom when a last-writer replay proves the
    windows cannot interleave. Interval-based coloring exists only as
    `unsequenced_spill_stages/stack_slot_coloring`, which
    `stage_register_allocation` never calls.
  - Composition (`src/register_allocation.rs`). A completed selected-lowering
    run takes `assignment::transformed` homes and surfaces pressure as
    `TransformedHomes`; it never enters `assignment::runtime_spill`. A
    selected-lowering selection beside an allocation-recovery selection, and
    two allocation-recovery selections, reject as `UnsupportedComposition`.
  - Stack provisioning under the
    [compiler-owned stack contract](wiki/spec/resources/storage.md#compiler-owned-stack-accesses).
    Reuse the final-frame demand path, not a parallel spill-byte estimate.
    `image-emission` derives each function's peak from its validated final
    frame (`src/function_fragments/production.rs`, replayed by
    `validation/stack.rs`) and composes it in
    `object_artifact/stack_demand.rs` and
    `installation_record/record_construction.rs::derive_installation_stack_demand`.
    External-root admission (`external-roots/src/root_entry/root_validation.rs`)
    and hosted-receiver partitioning (`image-emission/src/hosted_receiver.rs`)
    compare that demand with supply. No test carries a runtime-spilled
    function through this path to an admission or a rejection. Target-required
    probing and setup are frame/provisioning work, not an owner-blocked
    language choice.

  Acceptance: slot reuse is not double-counted; changed allocation or frame
  realization invalidates stale demand; insufficient supply rejects before
  execution; and generated loads and stores independently replay their
  physical geometry and value lineage without adding source crash routes. A
  byte ceiling alone does not stand in for valid stack backing. Keep
  `WRITE-ONLY-BORROW`'s source-backed three-call regression
  (`tests/native-differential/tests/terminal_psi_indexed_receivers/primitive_stores.rs`)
  as the hosted execution control, with its installed-image demand replay and
  stale-frame rejection (`terminal_psi_indexed_receivers/stack_pointers/`).

  Flag: `unsequenced_spill_stages/` holds 18 spill families in about 25,700
  non-test lines (logical spill operations, slot coloring, abstract
  insertion, reload-value homes, recursive and generalized worklists,
  pseudo-instruction lowering, memory effects, access constraints).
  `stage_register_allocation` calls none of them; native-differential tests,
  architecture ladders and machine emission's non-authoritative
  `frame_layout/spill_requirements/` are their only consumers. The executable
  route is about 1,100 non-test lines of allocation code plus 2,950 of
  rewrite, and slot reuse now has two owners. Sequence a staged family behind
  the executable route or delete it; do not extend both.

- **ALLOCATION-REFINEMENT.** Add general live-range splitting to
  [register allocation](omega-rust/omega/pipeline/selected-instructions-to-register-homes/README.md)
  while preserving exact register-unit aliases, liveness, and target custody.
  Copy-affinity coalescing in home assignment and in fixed/precolored segment
  homes, rematerialization ahead of private storage, and the fixed/precolored
  interval stages on the default route already exist. Fixed-use splitting in
  `selected-instructions-to-selected-instructions`:
  `src/rewrites/allocation_recovery/fixed_view_copy/` now has three legs —
  `LeafLocalBeforeFixedUseV1` copies a `u64` `EntryParameter` live-in before a
  `Return` operand in a leaf block, `ImmediateBeforeFixedUseV1` copies any
  scalar source-value origin in the boundary's own block immediately before
  each pinned operand-Use site (ordinary or terminator position), admitting
  chained pinned segments, and `SharedSourceExitBeforeFixedUseV1` — the
  default path — partitions the boundaries of one register, source segment
  and domain, and view transition, and emits one copy per partition unit:
  boundaries whose recorded fragments all enter through connectors out of a
  single block's shared terminator share one copy at that dominating
  source-segment end, cheaper than one copy per use, while every boundary
  lacking that connector evidence keeps a copy at its own site. The declared
  `SharedEntryAfterCompareBeforeBranchV1` selection runs the same partition
  under its entry-parameter admission gate — refusing any boundary the
  partition cannot share — rather than its former whole-function
  compare/branch/leaf template. Allocation legality licenses every operand
  Use site with all other views pinned on the register as candidate sources,
  so a value used at two incompatible operand views allocates through
  recorded split copies and post-copy reanalysis instead of failing
  `UnresolvedEntryTransitions`; independent replay rebuilds each emission —
  shared exit or site copy — from current facts. Splitting still fires only
  on declared fixed-use boundaries — there are no allocation-chosen split
  points inside a pressure region, and other pressure cases go to
  rematerialization or runtime spill.

  Remaining work:

  - Split a live range at allocation-chosen points and home each segment
    independently, across a pressure region rather than only at pinned
    operand uses: the shared-exit leg already chooses one dominating copy
    over a fixed-use fan-out; what remains is choosing split placement and
    connecting copies inside a pressure region with no pinned sites, then
    accepting homes only over fresh liveness, ranges and legality.
  - Lift the limits in `src/analyses/fixed_precolored_split_requirements/`.
    The cross-block topology now admits a disjoint union of source-rooted
    fragment trees: a fragment with no incoming connector opens a fresh
    component — edge parameter bindings leave the target parameter's fragment
    live-in without one — and a connector ending at a fragment-less block is
    a tolerated transport exit, so a join binding each arm's argument to the
    parameter partitions and homes cleanly. Tied registers, early-clobber
    domains, joins of two connectors into one fragment target, and cycles
    still reject as `UnsupportedTiedRegister`,
    `UnsupportedEarlyClobberDomain` and `UnsupportedCrossBlockRange`
    (`compute/partition.rs`, `compute/topology.rs`).
  - Place splitting in the recovery order.
    `assignment/runtime_spill/recovery.rs` tries rematerialization, then a
    call-crossing spill, then a bounded spill; a split that frees a home
    without storage has no position there.

  Acceptance: a value other than a leaf-returned entry parameter, with uses on
  incompatible views or across a pressure region, allocates through recorded
  split points and per-segment homes, and replays through callable
  publication on every admitted target. Independent replay reconstructs the
  split points, copies and segment homes from current facts. A moved or
  omitted split point, a copy across an incompatible fixed-use boundary, a
  segment home that aliases a live register unit, and stale post-split
  analyses reject. `SPILL-REALIZATION` owns private storage; this item adds
  no slot.

  Resolved flag: the `SharedEntryFixedViewCopyAfterCompareBeforeBranchV1`
  whole-function template is replaced by the shared-source-exit partition —
  `fixed_view_copy/emission.rs` groups boundaries by register, source segment
  and domain, and view transition and emits one copy per unit, so the
  declared selection keeps its entry-parameter gate while the copy-placement
  decision (one copy at a dominating shared segment end when cheaper than one
  per use) lives in the default path, not in a sibling per CFG arrangement.

- **FRAME-LAYOUT.** Complete exact nonzero-frame realization. Red-zone policy,
  stack probing, unwind information, stable-address loans, the general-call
  frame/callee-save/link-register/call-site-alignment plans, and the
  realization stage's frame-policy rosters all replay through ordinary
  callable publication on the admitted targets.

  Remaining: dynamic-allocation constraints, blocked upstream. Every selected
  local and outgoing slot resolves to a static byte extent, so there is no
  runtime-sized stack allocation to constrain. This leg waits on a
  language and Terminal Psi contract for runtime-sized activation storage,
  not on frame-layout work; open that contract before resuming here.

  Acceptance: a program with a runtime-sized activation allocation publishes a
  frame whose committed extent, probe roster and unwind information replay
  against the validated layout on every admitted target, and a suppressed or
  invented extent replays false.

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the symbolic instruction-pair
  descriptors in `selected-instructions-to-selected-instructions`
  (`src/rewrites/selected_lowering/literal_fold/pair_rule.rs`) to physical
  register units, effects, traps, memory, stack, and control flow without
  replacing the independent validator: `literal_fold/validate/replay.rs`
  restates every grammar from instruction records and never reads a
  descriptor. `SelectedInstructionPairRule` declares producer, consumer and
  rewritten kinds, operand shape, immediate bound, result disposition, unit
  effects and machine effects, and the
  [exact-rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  lists the 23 landed `SelectedIncoming*` selections. Every landed pair
  eliminates an effect-isolated `MaterializeI64` that immediately precedes its
  single consumer in the same block. The candidate is one pressure-recovery
  `ImmediateU64RematerializationCandidate` per function and fixed-point
  iteration (`compute/actions.rs::derive_action`), under the caller-saved-only
  availability policy
  (`src/analyses/legality/policies.rs::frameless_leaf_caller_saved_views`).
  Every rewritten form except the indexed-read fold carries no memory, trap,
  stack or control effect.

  Remaining work:

  - Unit roles. `PairUnitEffects` requires a rewritten row with no implicit
    uses or clobbers and rejects every `tied_to` operand, and every
    `PairMachineEffects` variant rejects a consumer with implicit unit uses.
    Landed relationships cover implicit definitions as the result channel,
    retired dead definitions (`DeadConsumerUnitDefs`) and operand-swapped
    definitions kept for equality readers (`OperandSwappedUnitDefs`).
    Flag-consuming forms, clobbering rewritten rows and tied operands have no
    declaration.
  - Traps. `FaultDischargedByLiteral`, `FaultDischargedByObligation` and
    their two `...DeadUnitDefs` forms retire a consumer fault that the folded
    literal or a carried obligation makes unreachable. None admits trap
    preservation, where the rewritten form keeps the consumer's
    `MayArchitecturalFaultV1` surface, or hosted-trap effects.
  - Memory. `IndexedPointerReadFold` is the only memory relationship: one
    indexed pointer read folded to an offset read of the same bytes. A pair
    whose consumer or rewritten form writes memory has no declaration.
  - Stack and control flow. No relationship exists. Every variant requires
    alternatives that leave the stack unchanged and fall through, and neither
    instruction of a pair may be a terminator or sit in another block.

  Acceptance: each dimension above has a declared relationship with a firing
  rule on x86-64 and AArch64, a validator restatement that reconstructs the
  relationship from instruction records alone, the corruption, wrong-policy,
  boundary and budget negatives required by
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage),
  and one compiler-generated publication replay. A relationship that cannot
  be replayed independently stays rejected. Another literal identity over
  already-declared dimensions does not advance this item.

  Flag: the descriptor grows by enumeration, not composition.
  `PairOperandShape` has 12 variants that are hand-written products of literal
  position, result kind and tail-operand custody (for example
  `BinaryLeftLiteralConstantResultAuxiliaryUsesOrScratchDefs`).
  `PairMachineEffects` spells fault discharge times dead definitions as
  `FaultDischargedByLiteralDeadUnitDefs` and
  `FaultDischargedByObligationDeadUnitDefs`. The validator's `SourceShape` has
  30 variants, one per identity and operand position. Each of the 23
  selections also takes an `Optimization` tag and a `LiteralFoldPolicy` bit,
  and the per-identity test files run 566 to 1,292 lines. More than 20
  commits each added one identity or operand position while stack, control
  flow and trap preservation stayed at zero relationships. The general
  mechanism is independent descriptor axes
  (literal position, result kind, tail custody, fault discharge,
  implicit-definition disposition) that compose, so that a new identity is a
  catalog row and not a new variant, policy bit, validator shape and
  vocabulary tag.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Execute copy removal, redundant-extension
  removal, address folding, compare/test selection, and scheduling on
  compiler-produced selected programs, each as an
  [atomic candidate with independent validation](wiki/spec/build/optimizations.md#atomic-candidates-and-independent-validation).
  Owner: `omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/`.
  `src/rewrites/` already holds 37 modules for these transformations:
  `copy_removal`, `redundant_extension`, `address_fold`, `literal_arithmetic`,
  seven compare and flag rewrites (`literal_*`, `constant_*`, `boundary_*`,
  `dead_compare`), and 26 interchange and relocation families. Each entrance
  takes caller-named instruction identities and replays by
  restore-by-content. None is an `Optimization` member, has a catalog row or
  candidate discovery, or has a caller outside its own tests, which
  hand-build `SelectedInstructionPlan` fixtures. `src/selected_optimization.rs`
  runs only the identity route or the selected-lowering literal folds, so
  none of them has changed a compiled program.

  Remaining work:

  - Give the stage an execution route under
    [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay):
    exact selection names, one ordered catalog whose descriptors retain rule,
    validator, policy, analyses, invalidations, budgets and applicability,
    candidate discovery that binds source and selection identities, and the
    call from `optimize_analyzed_selected_instructions`. Empty and nonempty
    selections stay on one physical route. Decision rows and receipts must
    survive replay through allocation and emission.
  - Separate validation from proposal. Every module's `validation.rs` calls
    the same `admission::admit` as its `rewrite.rs`, then checks that undoing
    the edit restores the source. That detects a wrong edit, not a wrong
    legality decision. The validator must reconstruct the preconditions
    without the producer's admission routine.
  - Scheduling refuses any window containing a call, hosted effect, barrier
    kind or call-roster entry, any cross-block move through a block that is
    not a plain `Source` block, and any control-flow shape without its own
    family. Replace the per-shape families with one relocation admission
    before covering more shapes (see Flag).
  - Compare/test selection: `SelectedInstructionKind` has three compare kinds
    and no bit-test kind, and `literal_minuend` admits only equality readers
    because no reversed ordering predicate exists. `copy_removal` substitutes
    within one block only. `address_fold` needs the `AddressOffset` producer
    in the consumer's block and now applies the plan architecture's
    displacement bound — AArch64's scaled 12-bit immediate or x86-64's
    disp32 — rather than sharing the scaled bound on every target.

  Acceptance: source-produced programs select each rule by exact name through
  `optimize_selected_instructions`, execute natively on a supported host, and
  replay independently after publication. The empty selection and each
  disabled rule reproduce identity output. Include one valid window that no
  current shape enumerates, and rejections for a register or condition-state
  hazard, a crossed call or hosted effect, a non-commuting memory access, a
  boundary settlement inside the window, a stale candidate, an exhausted
  budget, and a legality error the producer accepts but the validator must
  refuse. A hand-built plan passing its own module's test is not the customer.

  Flag: scheduling has grown one family per window shape: five in-block
  moves, each repeated as a commuting variant, ten cross-block shapes
  (`edge`, `predecessor`, `diamond`, `join`, `fork`, `arm`, `bypass`,
  `triangle`, `confluence`, `inflow`), and six of those repeated for runs.
  That is about 64,000 lines, 49,000 of them tests, and the product of shape,
  member or run, and plain or commuting is still open. The families share
  `window_hazards.rs`, `block_edges.rs`, `dead_path.rs` and
  `commuting_accesses.rs` and differ only in how they locate the window. The
  general mechanism is one relocation rule over a member run and a
  destination point that derives the crossed positions and edges on every
  path between them and the traversals that gain or lose the run, then
  applies the hazard, dead-path and commutation audits once. Separately,
  `literal_compare` and `literal_arithmetic` re-implement folds the cataloged
  pair rules already produce; widen candidate nomination for those
  descriptors under DECLARATIVE-PEEPHOLES instead of keeping a second
  producer.

  DECLARATIVE-PEEPHOLES owns the cataloged
  pair-rule folds. ALIAS-AWARE-MEMORY owns the load, store and mutation
  rewrites in the same directory. PER-RULE-COVERAGE owns the disabled and
  downstream-replay legs once these rules are selectable.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Execute borrow-aware load forwarding, dead-store
  elimination, and mutation motion on compiler-produced selected programs,
  with non-aliasing justified by retained ownership evidence under
  [evidence and control flow](wiki/spec/build/optimizations.md#evidence-and-control-flow).
  Owner: `load_forwarding`, `dead_store` and `store_motion` in
  `omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/`.
  Each entrance takes one caller-named load or store, decides interference
  from the validated `memory_accesses` roster, walks across edges, and replays
  by restore-by-content. They handle exact-width `Store` and `StorePacked`,
  the place's own local slot, byte-sequence stores, indexed byte loads, and
  constant-count `CopyBytes` covers. None is an `Optimization` member, has a
  catalog row or candidate discovery, or has a caller outside its own tests,
  which hand-build `SelectedInstructionPlan` fixtures.

  Remaining work:

  - Bind the non-aliasing premise to evidence. `interferes` in each
    `admission.rs` treats a row naming another `PlaceId` as unable to observe
    or disturb the subject, on the documented premise that exclusivity was
    enforced before selection. `SelectedMemoryAccess` and the three receipts
    carry no loan, compatibility-certificate or accepted-fact identity for
    that premise, and [loans](wiki/spec/terminal-psi/loans.md) states that a
    live exclusive loan does not prove projections disjoint. Either retain
    the consumed fact identities in candidate and receipt, or establish at
    the roster's producer, under independent validation, that distinct
    `PlaceId`s in one function never overlap. `AnalysisKind::PlaceAliases`
    and `MemoryVersions` are declared in `optimization-core` and have no
    producer.
  - Add the execution route and separate validation from proposal as
    EXACT-MACHINE-SIMPLIFICATIONS describes. All three `validation.rs` files
    call their own rewrite's `admission::admit`.
  - Remaining refusals: forwarding into a join whose legs stored different
    registers, which needs a merged value on the edges; store motion through
    a fork or into a join, where the store lands at the last proven position;
    a dynamic dead extent without the byte-exact sequence write; a span whose
    count is not a materialized literal; a constant-index sequence row that
    lands off the dead byte still interferes; stores into `Structural`
    staging slots are not candidates.

  Acceptance: source-produced programs select each rule by exact name through
  `optimize_selected_instructions`, execute natively on a supported host, and
  replay independently after publication. The empty selection and each
  disabled rule reproduce identity output. Negative controls: a read through
  a shared reborrow or field projection of the stored place between two
  stores (the first store survives), an intervening call or hosted effect, a
  volatile or placed access, a partial overlap, a stale candidate, an
  exhausted budget, and one-field corruption of the receipt's fact
  identities. A hand-built plan is not the customer.

  Flag: byte-extent reasoning has one copy per rewrite. `intersects`,
  `reached_by` and `interferes` are defined in all three `admission.rs`
  files (the `dead_store` and `store_motion` copies of `interferes` differ
  only in the subject's name), the place, packed and local store-shape
  recognizers twice, and `commuting_accesses.rs` holds a fourth disjointness
  decision. Each access kind (packed, local slot, byte sequence, byte span)
  was then added to each rewrite as its own role-by-route case. The general
  mechanism is one owner that maps a roster row to place, storage route and
  extent (exact range, lower-bounded dynamic reach, or base plus index value)
  with may-overlap, must-cover and must-equal relations, consumed by the
  three rewrites and the commutation audit. `place_storage.rs` is the start
  of that owner.
- **REPRESENTATION-SPECIALIZATION.** Add field/variant relevance and
  invariant-window specialization.
- **CLEANUP-PRUNING.** Add cleanup and transition reachability pruning without
  losing affine/linear custody.
- **STATE-SPECIALIZATION.** Add state-argument/result specialization with exact
  edge provenance. One bounded family exists in
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/state_specialization/`:
  an unconditional `Jump` that binds the parameter of a single-`Conditional`
  dispatch block to a proven Boolean constant is fused with the resolved arm
  and carries both edges' provenance and fuel settlements, with separate
  proposal, validation and application. `lib.rs` exports it, but it has no
  `PSI_PASS_CATALOG` entry, selection name, or caller outside its tests. It
  declines every machine holding a cyclic component and does not cover
  non-Boolean state arguments, conditional incoming edges, or result
  specialization.
  Acceptance: a source-produced state machine selects the rule by exact name
  through `optimize_abstract_operations`, publishes, and replays
  independently. Forged or stale edge provenance, a dispatch whose every
  incoming edge is constant, and a disabled selection behave as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires.
- **INTERPROCEDURAL-SUMMARIES.** Add proof-bound inlining and the service/call
  summaries it needs. Transitive per-function effect summaries (observable,
  structural-state, crash, suspension, services, boundaries) and the direct
  call graph already exist in
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/`
  and feed control-flow cleanup and loop-invariant scalar motion. They
  classify effects only: no summary names the places a callee reads or
  writes, and no rule inlines or otherwise rewrites across a call.
- **PROOF-DIRECTED-LOOPS.** Add loop-bound reasoning, induction
  simplification, and vectorization with exact lane semantics.

## Verification and rollout

- **PER-RULE-COVERAGE.** Keep positive, negative, boundary, disabled, budget,
  determinism, fixed-point/idempotence, and corruption coverage complete for
  every exact rule, as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires. Do not call repeated reconstruction idempotence when the published
  artifact is not a legal second input. Every row of the checked
  [rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  has those legs beside its rule, as do the mandatory `runtime_spill` and
  `runtime_rematerialization` recovery rewrites, which have no disabled axis
  by design. The 40 uncalled rewrite modules under
  `selected-instructions-to-selected-instructions/src/rewrites/` (the
  EXACT-MACHINE-SIMPLIFICATIONS and ALIAS-AWARE-MEMORY families) have
  positive, negative, boundary, budget, determinism, fixed-point and
  corruption legs on hand-built plans only. They have no disabled,
  exact-selection, empty-identity, unsupported-composition or
  downstream-replay leg, because none has a selection name or a stage caller.
  An isolated applied-rule test does not establish compiler-generated
  application or publication support.

  Remaining work:

  - As those two items give a module a catalog row, add the missing legs
    through `optimize_selected_instructions` and the native-differential
    suites: exact selection, disabled and empty selection producing identity
    output, rollback by `--disable-optimization`, unsupported composition,
    and replay after allocation and emission.
  - New exact rules in any phase land with the full matrix; this item does
    not list them. `TargetOperations` and `PreAllocation` are empty identity
    boundaries today.

  Acceptance: every inventory row and every rewrite reachable from a stage
  exercises each axis through its stage's public entrance, with the published
  artifact as the second input for the fixed-point leg. No rule counts as
  covered while an axis is recorded as absent.

  Flag: coverage is maintained by hand. Under `rewrites/`, 45 test modules
  each define their own `budget()`, 44 their own `instruction()` fixture, and
  37 rewrite modules their own measured-boundary test, about 119,000 test
  lines in total; this entry tracked the result as prose. No check fails when
  a rule lacks an axis. `tests/architecture/optimizer_rollout` already
  derives the rule set from `Optimization::ALL` and the stage catalogs and
  reconciles names, phase, applicability and rollback. The general mechanism
  is a checked per-rule axis table in that gate, or one shared matrix harness
  parameterised by a rule fixture, so that a missing axis fails the
  repository gate.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target. The format
  and first row landed: `omega-benchmark-record/1` in
  [tools/benchmark](tools/benchmark/README.md) (stdlib-only `benchmark.py
  prepare`/`measure`/`validate`; `prepare` settles the package-review gate
  by accepting the generated review and publishing the host-local
  `omega.lock`), pinned by `tools/tests/test_benchmark.py`, with
  `records/cli_mvp__linux_x86_64__default.json` measured at
  87d8b22713 on a Linux x86-64 host (dev-profile `omega`, 3 compile +
  5 run samples, exit 0). Remaining: rows for further subjects and
  nonempty selections through the same `measure` command, and the
  windows_x86_64, macos_arm64, linux_arm64, and uefi_x86_64 legs on
  matching hosts — unavailable on this host and recorded as such in
  [wiki/drafts/benchmarks.md](wiki/drafts/benchmarks.md).
