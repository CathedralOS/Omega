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
governs these tasks: replace obstructive implementations, preserve semantics and
independent validation, and keep empty/nonempty optimization selections on one
physical route. Unsupported cases reject rather than restoring a fallback.

- **PIPELINE-OWNER-CONSOLIDATION.** Finish ownership and executable consumption
  under the [connected pipeline contract](omega-rust/pipeline.md). Selected
  rewrites need the stage catalog routes owned by EXACT-MACHINE-SIMPLIFICATIONS,
  ALIAS-AWARE-MEMORY, and DECLARATIVE-PEEPHOLES, or deletion when superseded.
  SPILL-REALIZATION owns joining retained logical spill/coloring evidence to
  physical recovery and retiring unused spill boundaries with their consumers.

  The semantic-wrapper encoding and object stages under
  `native-realization/src/optimized_semantic_wrapper_{encoding,object}/`
  have production callers in `native_realization/optimized_fragment_projection.rs`.
  Move their coupled record, codec, validation, and backend responsibilities
  out of the coordinator without breaking that route. UEFI/provider/ABI gaps
  remain with their native owners, not another wrapper implementation.

  Audit surviving public entrances using qualified identities and repository-wide
  consumers, including native-differential tests; a common name such as
  `encode` does not identify a caller. Acceptance: the connected route has no
  competing entrances or orphan outputs, coordinators sequence typed stages,
  and retained plans constrain the physical operations they describe.
  An isolated validator or retained-but-unused plan does not close the join.

- **REPRESENTATION-OWNERSHIP.** Finish durable representation ownership for
  the semantic-wrapper record and codec under
  `native-realization/src/optimized_semantic_wrapper_object/`, coordinated
  with PIPELINE-OWNER-CONSOLIDATION's coupled wrapper disposition.
  Follow [representation ownership](omega-rust/omega/representations/README.md):
  current program data outlives its producer; historical inputs remain explicit
  replay evidence, not the route to current data.

  Acceptance: durable records/codecs live with their representation owners,
  ordinary consumers read current data directly, replay inputs remain distinct,
  and `tests/architecture/representation_ownership.rs` covers the resulting
  named roots and ownership. Preserve direct-read controls and legitimate
  retained proof-input identity checks. The post-allocation manifest and
  fixed-view-copy codecs already live in register-homes; do not relocate again.

## Product pruning and rollout

- **WORKSPACE-ROLLOUT.** Keep exact rules opt-in and Experimental in the
  [rule inventory](omega-rust/omega/representations/optimization-core/rules.md)
  until the frozen-tree command
  `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace --no-fail-fast`
  passes and promotion receives the separate owner decision required by the
  [promotion contract](wiki/spec/build/optimizations.md#release-rollback-and-promotion).
  Do not substitute `--all-targets`, which omits doctests.

  The six [staged promotion records](omega-rust/omega/representations/optimization-core/promotions/)
  contain rollback and Linux measurement evidence; Approved status and Owner
  approval remain PENDING. Review evidence against the contract rather than
  treating populated fields or a schema-gate pass as approval or broader host
  coverage. Keep inventory and records consistent under
  `exact_rule_rollout_is_complete_and_promotion_gated`.

  Acceptance: the command passes from a clean checkout, and each promoted exact
  rule has the required reviewed evidence and owner decision. No broad
  optimization level or automatic default is introduced.

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
    `ExactCompilerIntrinsic`, as an FMA span, an IEEE float-compare
    fragment, and the three integer-comparison kinds (`IntegerEqual`,
    `IntegerLessThan`, `IntegerLessOrEqual`), whose instruction-only
    operations take `DirectInstructionBytes` over their
    provenance-attributed byte interval when the realization carries a
    `PrimitiveIntegerComparison` execution. Landed (w9, `95019d341a9`):
    `CallDynamic*` kinds now carry occurrences and children — physical
    derivation enumerates every surviving `CallDynamicScalar`,
    `CallDynamicParameterScalar`, `CallDynamicUnit`, and
    `CallDynamicParameterUnit` Terminal operation as a `DynamicCall`
    occurrence, each binding exactly one child under
    `PhysicalChildParent::DynamicCallDispatch` (the exact dispatch catalog
    row the operation names), and `derive_dynamic_call_span` joins all
    five emitted record families — `dynamic_calls`,
    `stored_dynamic_calls`, and `dynamic_parameter_calls` take
    `DirectInstructionBytes` over the register-indirect interval
    (rejecting empty or relocated spans) while
    `forwarded_dynamic_parameter_calls` and
    `forwarded_dynamic_descriptor_calls` require exactly one Text
    relocation plus an exact callee join for `ResolvedInternalCall`.
    Witness `derivation::tests::dynamic_call_occurrence_binds_its_dispatch_role_and_parent_identity`.
    Re-verified at `138ed79a677` for mined stub
    **DYNAMIC-CALL-OCCURRENCE-SPANS** (resolved — this landed slice is the
    whole item: `CallDynamic*` occurrences enumerated in
    `physical/projection.rs`, span join in `derive_dynamic_call_span`, witness
    test PASS on linux x86-64). Residual stays here: descriptor-materializing
    relocation custody (fenced PHYSICAL-ACCESS-PROFILES) and the e2e replay leg
    (dynamic-call programs red before the physical stage upstream).
    z98 side (`9e386769132`, rebased — code superseded by upstream
    `da97c882017`'s generalized all-family custody): the forwarded
    `forwarded_dynamic_descriptor_calls` span the complete emitted record
    with multi-window relocation custody — the resolved call relocation plus
    every descriptor argument's table-address windows (one
    `X86_64Relative32`, or the AArch64 `Aarch64Page21`/`Aarch64PageOffset12`
    pair), each attributed to the call's operation and joined to the
    conformance table whose application the argument names; any other
    relocation overlapping the record rejects as unattributed, and
    `derive_span` carries a relocation-window set rather than the single
    window it modeled. Witnesses
    `derivation::tests::dynamic_call_occurrence_binds_its_dispatch_role_and_parent_identity`
    and `operator_applications::tests::descriptor_table_relocations_*`.
    Remaining intrinsic kinds still produce no occurrences, so their
    span arms have no demand side — the occurrence replay for them is
    TV-OPERATOR-APPLICATIONS-REPLAY's scope.
    Measured (w9, `577d6ac2ba`): no end-to-end `physical_child_replay`
    leg for the dynamic family is reachable yet — dynamic-call programs
    are red before the physical stage on this host
    (`runtime_local_named_dyn_unit_multi_hop_return` rejects
    `CallUnitWithDynamicArguments` under Selection/Legalization; the
    rebound and stored shapes hit the `ProgramEntry establishment
    rejoins 0 Terminal attachment identities` family), so the family's
    e2e replay leg waits on those upstream gaps. Regressions:
||||||| parent of d7744b8fbcab (board: TRANSLATION-VALIDATION — re-census physical/ fence, pin descriptor-custody leg shape)
    `PrimitiveIntegerComparison` execution. `CallDynamic*` kinds carry
    descriptor or parameter ordinals rather than a static callee, and
    the remaining intrinsic kinds have no span arm. Verified
    (w9, `fcef01c59a`): those operations do not produce coverage
    occurrences at all yet — the checked boundary-operator replay
    (`lowered-psi-to-terminal-psi/boundary_operator_custody/replay_scope.rs`)
    admits only IEEE FMA, structural returns, and float/integer
    comparisons — so the first work is a new occurrence replay family
    with demand/realization companions; only then do span arms join the
    emitted dynamic-call records (`dynamic_calls`, `stored_dynamic_calls`,
    `dynamic_parameter_calls`, `forwarded_dynamic_*`), which already carry
    `psi_operation`/`operation_ordinal`/`code_offset`/`byte_count`.
    Descriptor-materializing records additionally need relocation custody
    beyond the single window `derive_span` models (AArch64 table
    addressing emits two windows) plus a conformance-table symbol join;
    parameter-routed calls are register-indirect. `physical/` is fenced
    by DYNAMIC-CALL-OCCURRENCE-SPANS this wave. Regressions:
    `PrimitiveIntegerComparison` execution. `CallDynamic*` kinds carry
    descriptor or parameter ordinals rather than a static callee, and
    the remaining intrinsic kinds have no span arm. Verified
    (w9, `fcef01c59a`): those operations do not produce coverage
    occurrences at all yet — the checked boundary-operator replay
    (`lowered-psi-to-terminal-psi/boundary_operator_custody/replay_scope.rs`)
    admits only IEEE FMA, structural returns, and float/integer
    comparisons — so the first work is a new occurrence replay family
    with demand/realization companions; only then do span arms join the
    emitted dynamic-call records (`dynamic_calls`, `stored_dynamic_calls`,
    `dynamic_parameter_calls`, `forwarded_dynamic_*`), which already carry
    `psi_operation`/`operation_ordinal`/`code_offset`/`byte_count`.
    Descriptor-materializing records additionally need relocation custody
    beyond the single window `derive_span` models (AArch64 table
    addressing emits two windows) plus a conformance-table symbol join;
    parameter-routed calls are register-indirect. `physical/` is fenced
    by DYNAMIC-CALL-OCCURRENCE-SPANS this wave. Re-censused at
    `ed566863a7c6` (04:12Z): the `physical/` fence drained — no live claim
    touches `native-artifact/src/physical/` or names DYNAMIC-CALL-
    OCCURRENCE-SPANS / PHYSICAL-ACCESS-PROFILES, so the descriptor-custody
    leg is implementable now. Its shape mirrors the existing two-window
    precedent `NormalizedForeignCallbackRelocations::Aarch64PageAddress`
    (page + page_offset) in `physical/model.rs`: a new
    `PhysicalRelocationDisposition` variant for forwarded descriptor
    calls carrying the callee relocation plus the descriptor-table
    materialization window(s) and the conformance-table symbol join, with
    matching evidence hashing in `derivation/evidence.rs` (`relocation_kind_tag`
    tags are a closed 1-4 range today). Regressions:

    `physical_child_replay::structural_result_operator_occurrence_replays_one_exact_physical_child`
    (Linux x86-64) drives a structural-result boundary operator through
    emission, exact-child binding, and every mutation-class rejection;
    `physical_child_replay::integer_comparison_occurrence_replays_one_exact_physical_child`
    (Linux x86-64) does the same for a compiler-intrinsic `==`.
  - Boundary settlements. `derivation/evidence.rs` joins each installed
    settlement's closed `(CompilerBuiltinExecution, BoundaryRealization)`
    pair against `HOSTED_BUILTIN_SETTLEMENTS` in `derivation/children.rs`
    (`HostedExitProcessI32`, `HostedWriteByteI32`, `HostedReadByte`), where
    one row declares the builtin's supported targets, admitted
    scalar-argument forms, and result custody for the shared
    `derive_hosted_builtin_child` span join — then the admitted-provider
    settlement, then the normalized foreign call. The catalog is complete
    against the closed three-variant `CompilerBuiltinExecution`; a fourth
    hosted builtin is one enum variant plus one catalog row, not standing
    work. Any other builtin, and any occurrence carrying neither an
    installed settlement nor a foreign call, yields no evidence.
  - Privileged port effects. Implemented: every retained effect must be
    consumed by an exact `MetadataOnlyPort` settlement join; one unowned
    effect drops the artifact's evidence as an `UnownedPortEffect` gap.
  - General calls wait on `FRAME-LAYOUT`, itself blocked on a contract for
    runtime-sized activation storage. That contract now verifies as
    held-by-construction (RUNTIME-SIZED-ACTIVATION-STORAGE-CONTRACT
    resolution), so FRAME-LAYOUT's remaining blocker is its own scope.

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

- **CUSTODY-MUTATION-COVERAGE.** Finish migrating legacy custody mutation
  matrices to `psi/foundation/mutation-matrix`'s inventory and substitution
  driver, preserving each family's independent checker.
  Remaining surfaces include component-publication tests, executable-installation
  tests, topology custody substitutions, and Terminal codec artifact matrices
  not yet using the driver. `optimization-core` re-exports the foundation
  harness; Psi consumers use its foundation owner directly.

  Reuse `custody_field_inventory!`, `run_one_field_substitution_matrix`, and
  the `custody_mutation_matrix` architecture gate. Nested installation,
  optimization-execution custody, and trust-graph custody already use the driver;
  do not repeat those migrations.

  Acceptance: every representable field changes independently, the containing
  identity recomputes honestly, and independent replay rejects the substitution.
  Unrepresentable fields reject at canonical encoding and are named explicitly.
  Audit record families for missing inventories separately: the gate rejects
  declared inventories without matrix consumers and macro-derived inventories
  without shared-driver consumers. New record families arrive with matrices under
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage).
  Shared fixtures must not replace the independent check with producer admission.

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

  - Other non-scalar families. `EstablishTrivialAffineLocal` relocates: the
    verifier admits cyclic-member establishments — the entry-prefix contract
    relaxes to one establishment site per local, member sites confined to
    self-reachable blocks — and the optimizer relocates the whole-place
    establishment behind `invariant_trivial_affine_local_admission`, with the
    freeze replay re-deriving custody (member-internal edges keep the
    persistent place live, exits and member returns dispose it) and rejecting
    kept internal discards, dropped exit disposals, and stale frontier
    catalogs. `CallStructural` relocates only in
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
  inserts and independently replays the private stores and reload pairs.

  Remaining work:

  - Logical-to-physical join. `assignment/runtime_spill/recovery.rs` calls
    `sequenced_logical_operations` to choose logical victims and plan their
    operations; recovery retains the result and replay recomputes it. The
    actual candidate loop still selects and rewrites victims independently.
    Connect the plan to the emitted spill/reload or frame obligations where it
    is needed, or remove redundant planning. Preserve executable recovery and
    independent checking; retaining another plan is not completion. A mismatch
    with the actual emitted operations must reject, not merely a changed copy
    of the retained plan.
  - Victim and use admission (`rewrites/runtime_spill/admission.rs`). A
    foreign-class IEEE scalar reaches its slot through the frame rows'
    shared carrier class: stores prepend `Float*ToBits`, reloads append
    `BitsToFloat*`, and a missing or impure conversion row keeps the victim
    a candidate-local rejection. Still rejected: a foreign-class victim without
    that transport pair (vector-class values,
    non-IEEE scalars); an early-clobber write tied to a victim use; an
    entry-bound victim when an edge targets the entry block; and a
    multi-chunk stored snapshot whose chunk loads are pinned or separated by
    a unit-writing instruction. Recovery then tries the next roster
    candidate and fails when the roster is exhausted.
  - Slot assignment. Every victim declares a private eight-byte
    `LocalStorageSlotId::Spill` slot. `runtime_spill/slot.rs` shares an
    existing slot only for the zero-offset `Store64` or
    `FrameAddress`-plus-`Load64` idiom when a last-writer replay proves the
    windows cannot interleave. Interval-based coloring is sequenced as
    `assignment/stack_slot_coloring`: `stage_register_allocation`'s
    runtime-spill recovery colors its retained logical-operation plan and
    replays the coloring, but emission still consumes only the per-step
    `LocalStorageSlotId` homes — the colored plan is validated evidence, not
    yet a physical input.
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

  Flag: the remaining `unsequenced_spill_stages/` families have test,
  architecture and non-authoritative frame-planning consumers, not an
  executable allocation route. Logical planning has moved out but still needs
  the physical join above. Slot reuse still has two producers:
  `assignment/stack_slot_coloring` (sequenced, retained evidence) and
  `runtime_spill/slot.rs` (drives the emitted slot sharing). Follow consumers
  when sequencing a needed family or deleting a superseded one with its
  exports and tests; do not extend both implementations.

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
  lists the implemented `SelectedIncoming*` selections. Every landed pair
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
    `PairMachineEffects` composition rejects a consumer with implicit unit uses.
    Landed relationships cover implicit definitions as the result channel,
    retired dead definitions and operand-swapped definitions kept for equality
    readers, as declared by `PairUnitDefRelation`.
    Flag-consuming forms, clobbering rewritten rows and tied operands have no
    declaration.
  - Traps. `PairFaultDischarge` composes with the independent unit-definition
    disposition to retire faults discharged by a literal or carried obligation.
    This isolated arithmetic fault axis has no trap-preservation or hosted-trap
    relationship. The indexed-read relation already preserves its matching
    trap surface.
  - Memory. `PairNonUnitSurface::IndexedPointerRead` is the only memory relationship: one
    indexed pointer read folded to an offset read of the same bytes. A pair
    whose consumer or rewritten form writes memory has no declaration.
  - Stack and control flow. No relationship exists. Every composition requires
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

  Generalize candidate nomination beyond the single pressure-recovery candidate;
  retired literal producers covered ordinary non-pressure materializations too.
  Preserve the retired left-zero CompareI64Zero refinement: the current
  COMPARE_LEFT_IMMEDIATE_U12 descriptor chooses an immediate compare, not the
  zero-specific form. Add that behavior through the common descriptor/catalog
  route rather than restoring `literal_minuend`.

  Extend the existing independent axes in `PairOperandShape`, `PairUnitEffects`,
  and `PairMachineEffects` for these remaining relationships; do not reintroduce
  a product-of-axes variant for each combination. Independent replay still
  reconstructs the relationship from instructions rather than trusting the
  descriptor. Distinct mathematical identities still need checked semantics;
  another identity over existing axes does not close the missing dimensions.
  Separate repeated operand-position mechanics in replay's `SourceShape` from
  identity-specific mathematics without making the validator consume producer
  descriptors. Exact selection identities remain explicit.

- **EXACT-MACHINE-SIMPLIFICATIONS.** Execute retained copy, extension, address,
  compare/test, and scheduling rewrites on compiler-produced selected programs.
  Owner: `omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/`.
  Its public stage executes selected-lowering literal folds, not the remaining
  helper rewrite families.

  Add exact selection names, ordered catalog descriptors, candidate discovery
  binding source/selection identities, and execution from
  `optimize_analyzed_selected_instructions`. Retain rule/validator, policy,
  analysis/invalidation, budget, and applicability identities through allocation
  and publication under [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay).
  Preserve independently reconstructed legality; restore-by-content alone
  detects incorrect edits, not incorrect admission.

  Consolidate scheduling through the member-run/destination mechanism where
  semantics match. Commuting variants need their commutation premise: blindly
  using the common MemoryOrdering refusal disables valid cases. Fork migration
  must preserve admission when skipped-arm traversal consumes the path budget.
  Interchange swaps two runs and is not one relocation. Retain real call,
  effect, settlement, register, and memory hazards; do not delete checks to
  admit more source shapes.

  Remaining capability boundaries include cross-block copy substitution and
  compare/test vocabulary. Preserve target-specific displacement bounds.
  DECLARATIVE-PEEPHOLES owns general nomination of retired literal folds and
  left-zero compare refinement; ALIAS-AWARE-MEMORY owns memory rewrites.

  Acceptance: source-produced programs select each retained rule by exact name,
  execute on a supported host, and independently replay after publication.
  Empty/disabled selections reproduce identity output. Include a valid window
  not covered by existing shape families, plus register/condition-state hazards,
  crossed calls/effects, noncommuting memory, boundary settlements, stale
  candidates, exhausted budgets, and a producer legality error that independent
  validation refuses. Hand-built helper plans do not close this customer.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Execute load forwarding, dead-store elimination,
  and store motion through the selected-stage catalog with source-bound
  candidates, independently checked receipts, and publication replay.
  Owners: `load_forwarding`, `dead_store`, and `store_motion` under
  `selected-instructions-to-selected-instructions/src/rewrites/`;
  EXACT-MACHINE-SIMPLIFICATIONS owns their shared stage-execution join.

  Bind distinct-place non-aliasing to retained fact identities, or independently
  establish at the access-roster producer that the distinct places cannot
  overlap. SelectedMemoryAccess and rewrite receipts do not yet carry the
  loan/compatibility/fact identities used to justify that premise. Exclusive
  ownership alone does not establish projection disjointness; follow
  [loan semantics](wiki/spec/terminal-psi/loans.md), not differing PlaceIds.

  Preserve shared storage-route and extent semantics in `place_storage` across
  the rewrites and commutation checking. Remaining control-flow work includes
  forwarding where incoming stored registers differ and store motion across
  forks/joins without adding or dropping writes. Unsupported dynamic geometry
  must reject until justified; do not equate storage routes or reuse stale
  evidence. Existing off-range constant-index, Structural-slot, and same-extent
  CopyBytes cases are controls, not new implementation tasks.

  Acceptance: source-produced exact selections execute natively and replay
  after publication; empty/disabled selections retain identity output. Preserve
  reads through shared reborrows or field projections, intervening calls/effects,
  volatile/placed accesses, partial overlap, stale candidates, exhausted budgets,
  and corrupted fact-identity negatives. Source admission and the independent
  validator must each establish the required alias and byte-extent facts.
- **REPRESENTATION-SPECIALIZATION.** Add field/variant relevance and
  invariant-window specialization. The bounded representation families in
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/representation_specialization/`
  and
  `omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/field_value_specialization/`
  are now wired into the pass pipeline under one exact-name selection:
  `Optimization::RepresentationSpecialization` selects
  `omega.psi-pass.representation-specialization.v1`, which schedules
  `omega.psi-rule.case-membership-specialization.v1` then
  `omega.psi-rule.field-value-specialization.v1` through
  `PSI_PASS_CATALOG`/`optimize_abstract_operations`, publishing and
  replaying each commit independently under the evidence-matrix legs.
  A `StructuralCaseMembership` reading a place whose case the unit itself
  proves — established in the same machine by `EstablishScalarCase`, or
  declared under a closed `Sum`/`Mixed` roster of exactly one case — folds
  to a `BooleanConstant` carrying the proven verdict, with
  `omega.validator.case-membership-specialization.v1` re-deriving the
  proof. The roster basis covers places no producer can fix (parameters,
  block parameters, results, and non-`EstablishScalarCase` operation
  results), and a membership's non-empty path folds when the position it
  resolves to — a `Record`/`Mixed` common field, `FixedArray` element, or
  `Reference` referent — closes over exactly one case. It declines
  memberships whose observed position carries no proof — unestablished
  multi-case roots and paths ending on multi-case or non-structural
  positions.
  A `BooleanStructuralField`/`IntegerStructuralField` read whose stored
  scalar the unit proves folds to the matching `BooleanConstant`/
  `IntegerConstant`, with `omega.validator.field-value-specialization.v1`
  re-deriving the proof on three bounded bases: the place's
  `EstablishRecord` producer supplying a constant-resolving scalar
  initializer at an empty path, the place's `EstablishScalarCase` producer
  supplying one at a lone `Case` path matching `result_case`, or the
  resolved field's declared `BoundedInteger` bound closing over exactly
  one value. Constant resolution follows same-function block-parameter
  bindings transitively and rejects divergent or cyclic chains; reads on
  unproven places, non-singleton bounds, unsupported paths, and non-scalar
  fields stay observations. Machines holding cyclic components stay frozen
  byte-exact under both rules. Field relevance beyond these proven-scalar
  observation folds still lacks operand-substitution machinery, and no
  invariant-window operation reaches this stage yet; both remain open
  under this item.
  The bounded families' acceptance chains are witnessed: source-produced
  machines select the rules by exact name through
  `optimize_abstract_operations`, publish, and replay independently, with
  forged or stale membership and field provenance, disabled selection, and
  the cyclic freeze behaving as
  [validation when extending a stage](omega-rust/optimization.md#validation-when-extending-a-stage)
  requires.
- **CLEANUP-PRUNING.** Add cleanup and transition reachability pruning without
  losing affine/linear custody.
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

- **PER-RULE-COVERAGE.** Complete exact-selection, disabled/empty identity,
  rollback, unsupported-composition, and downstream-replay coverage as
  EXACT-MACHINE-SIMPLIFICATIONS and ALIAS-AWARE-MEMORY connect rules to their
  public stage entrance. Factor duplicated rule-fixture matrices through shared
  support without weakening positive, negative, boundary, budget, determinism,
  fixed-point, or corruption axes.

  Keep [inventory rows](omega-rust/omega/representations/optimization-core/rules.md)
  resolving to actual tests under
  `tests/architecture/optimizer_rollout/coverage.rs`. New exact rules land with
  the full [stage-extension matrix](omega-rust/optimization.md#validation-when-extending-a-stage);
  mandatory runtime recovery retains its explicit no-disabled-axis distinction.

  Acceptance: every executable rule exercises each applicable axis through its
  public stage entrance. Fixed-point tests use the published artifact as a legal
  second input, not repeated reconstruction. Absent axes need the gate's explicit
  justified disposition; hand-built helper tests do not establish
  compiler-produced execution or publication.

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
  nonempty selections through the same `measure` command. The
  e48558bd41 rejection — every `depend()`-ing subject failing native
  realization with `Terminal proposal must retain every integer
  comparison occurrence exactly once` — is lifted: at ff782bdf21 the
  `measure` compile leg over `cli_mvp`/linux_x86_64 with
  `--accept-admissions` publishes native output (the comparison-occurrence
  producer now covers std plumbing), so linux_x86_64 rows are unblocked.
  The remaining work is row authorship (`PRIME-COUNTER-BENCHMARK-ROW`
  owns `tools/benchmark` and `wiki/drafts/benchmarks.md`) plus the
  windows_x86_64, macos_arm64, linux_arm64, and uefi_x86_64 legs on
  matching hosts — unavailable on this host and recorded as such in
  [wiki/drafts/benchmarks.md](wiki/drafts/benchmarks.md).
