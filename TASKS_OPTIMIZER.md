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

- **REVIEW-RESEAL-ELIMINATION.** Remove repeated identity serialization of the
  same unchanged in-memory UEFI semantic-wrapper object across construction,
  encoding and staging validation in
  `native-realization/src/optimized_semantic_wrapper_object`.
  Construction already uses `validate_object_preserving_seal`; encoding and
  trailing staging validation still invoke full object validation. Trace those
  repeated computations before changing them. Preserve independent decoding,
  shape/target/template checks and honest-reseal mutation controls. Acceptance:
  demonstrate fewer duplicate identity computations on the actual staging route,
  identical valid artifacts and unchanged rejection of corrupted/substituted
  encoded objects. Do not remove decode-boundary checking or introduce
  package-acceptance receipts.

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

- **TRANSLATION-VALIDATION.** Complete source-to-physical occurrence coverage
  for admitted operations/transfers on the common graph under
  [closed application and physical occurrence](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence).
  Owners: `lowered-psi-to-terminal-psi/src/boundary_operator_custody.rs`
  and `native-artifact/src/physical/{operator_applications.rs,derivation/}`.

  For remaining intrinsic families, retain checked demand and exact Terminal
  occurrences before adding physical span arms. Current operator replay covers
  local initializers/FMA, structural returns and float/integer comparisons.
  Rebound scalar/unit calls now replay into `CheckedDynamicCallOccurrence`
  rows beside the D29 roster (descriptor-table lane); register-indirect forms
  remain. `derive_operator_physical_span` still returns `Ok(None)` — and the
  occurrence names `UnsupportedOperatorSpan` — for non-FMA/non-comparison
  `ExactCompilerIntrinsic` realizations and non-static-callee operations under
  checked-body realizations; no end-to-end gap has been witnessed because
  corpus fixtures currently omit unit plans upstream at ProgramEntry
  establishment (structural field store / transition scalar-argument phases,
  pre-existing on this base). Parked probe: branch
  `swarm/linw3-translation-validation` (`tests/linw3_translation_probe.rs`
  compiles a directed-float canary through package-input helpers and inspects
  `physical_evidence_gap()`). Dynamic occurrence families and
  multi-window relocation custody already exist; use their current tests and
  `physical_child_replay`, not the retired single-window model. Reproduce
  source customers and attribute any earlier refusal to its actual stage.

  Acceptance: each surviving occurrence binds exactly one physical child with
  nonempty machine/object/final-image spans. Missing, duplicate, stale,
  substituted, padded and role-swapped children reject; unsupported coverage
  names the exact occurrence rather than erasing all evidence. Preserve
  verified-elimination exemptions and exact port-effect settlement ownership.
  General calls do not depend on runtime-sized activation claims under
  FRAME-LAYOUT. Do not restore retired scalar/Unit/structural whole-function
  planners to recover coverage; extend ordinary operations and receiving checks.

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

- **JOIN-INVARIANT-REWRITE-TRANSPORT.** (new-scope) Decide whether a scalar
  block invariant can be rewritten alongside the parameters it names, or
  whether its presence must keep refusing the rewrite. `a0cc5b2830` began
  proposing an interval from literal arrivals at a scalar join
  (`checked-trees-to-lowered-psi/src/proofs/scalar_block_invariants/joins.rs`),
  so a merge block joining two same-type integer literals now retains a
  `ScalarBlockInvariant`. That makes the machine proof-bearing, and both
  copy propagation and dead pure scalar elimination then propose their full
  rewrite, fail validation with `ChangedProofQuestion`, and fall back to the
  authored module. The refusal is the documented boundary working, not a
  defect: the rewrite would move a question the invariant row still names.
  The cost is that an ordinary two-arm `transition` returning constants now
  disables both passes for its whole machine, silently and with no
  diagnostic.

  Remaining work:

  - Establish which of the two routes the boundary takes: transport the proof
    context with the rewrite so the invariant row follows its renamed values,
    or state that an invariant-bearing header is deliberately frozen and give
    the refusal an observable report so the loss is not silent.
  - Either way, cover the shape at the corpus level. The only fixture that
    exercised it is `dead_block_parameter_fixture` in
    `checked-trees-to-lowered-psi/src/tests/preterminal_optimization.rs`,
    which now spells its arm constants at the declared type's endpoints
    precisely so no invariant is proposed and the non-proof-bearing lane stays
    reachable.

  Acceptance: a two-arm transition returning constants either keeps both
  passes, or refuses them through a reported decision an owner can read, and a
  fixture pins whichever holds.

- **GENERAL-CYCLIC-EXECUTION.** Complete the receiving/native half of
  [ranked callees on projected receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers).
  The same-name TASKS.md item owns source production and verifier coverage.
  Natural-ranked and unranked Terminal programs use the common graph, with
  authenticated cyclic components frozen under optimization; retired
  countdown-only custody is not a fallback.

  `tests/native-differential/tests/terminal_psi_indexed_receivers/cyclic_receivers.rs`
  already compiles nested `root.child.walk`, surrounding caller stores,
  ranked/unranked backedges, four-target object publication and composed
  caller/callee stack demand, executing on its matching host. Retain this as
  the positive control, not an unimplemented whole-entry-only limitation.

  Close the remaining acceptance coverage: conflicting parent access,
  missing/invalid callee ranking, exact argument identity, return/cleanup replay,
  and native execution on both Linux architectures. Establish each against the
  current route before adding machinery. Fixed-referent loops do not establish
  broader changing-reference transfers; those need represented loans/aliasing
  and ranking substitution, not a parameter-count-only relaxation.

- **GENERAL-LICM.** Extend invariant motion beyond the authenticated
  shared-source preheader in
  `abstract-operations-to-abstract-operations/src/ranked_rewrites/loop_invariant_scalar_motion/`.
  Keep independent admission/replay in `src/validation/`, including invariant
  operand substitution and the existing topology-based non-speculation gate.

  Remaining boundaries:
  - Establishment/custody families not covered by current relocation.
    Copyable unrestricted whole-root Owned arguments are already supported;
    affine/linear argument transfers need custody the boundary cannot currently
    re-express. Plain unrestricted claim-free structural results already admit;
    call motion carrying claim transfers, requirement obligations, crash
    continuations or selected evidence still lacks reconstruction of those
    relationships. Missing cyclic source operations must first pass ordinary
    Psi verification, not synthetic bypasses.
  - Profitability beyond the current execution-guarantee gate, keeping logical
    fuel distinct from optimization cost under
    [optimization semantics](wiki/spec/build/optimizations.md).
  - Motion needing new blocks or run duplication rather than the existing
    shared-source preheader, with corresponding block/occurrence evidence.

  Acceptance: each transformation independently reconstructs components,
  loop-carried custody, ranking, provenance, effects and fuel. Forged operands,
  retained internal discards, missing exit disposals and stale frontiers reject.
  Rebuild ownership membership after transformation and preserve Terminal-derived
  region custody; do not add a private loop forest or restore countdown-specific
  authority.

## Register allocation and frames

- **SPILL-REALIZATION.** Finish executable pressure recovery and join its
  frames to stack provisioning.
  [Register allocation](omega-rust/omega/pipeline/selected-instructions-to-register-homes/README.md)
  chooses victims in `src/assignment/runtime_spill/`; the rewrite owner,
  `selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`,
  inserts and independently replays the private stores and reload pairs.

  Remaining work:

  - Logical-to-physical join. Resolved by removal: the retained logical
    spill-operation plan never described an emitted operation. The logical
    boundary models an evicted active resident with current/reclaimed views,
    while executable recovery spills the failed register — frequently the
    incoming value — directly; the planner rejected every observed runtime
    choice as `UnsupportedVictimRole` and no recovery retained a plan. The
    redundant plan, its coloring, and their replay comparisons were removed;
    physical candidate, rewrite, and rebuilt-fact replay is unchanged.
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

- **ALLOCATION-REFINEMENT.** Add allocation-chosen live-range splits inside
  pressure regions, beyond existing fixed-use and shared-source-exit copies.
  Owners: `selected-instructions-to-register-homes`'s
  `analyses/fixed_precolored_split_requirements/{compute,replay}/` and
  `assignment/runtime_spill/recovery.rs`, plus selected-instruction
  `rewrites/allocation_recovery/fixed_view_copy/`.

  Choose split points, connect independently homed segments, and recompute
  liveness, ranges and legality. Place splitting in recovery before private
  storage when it can free a home. Extend tied-register, early-clobber,
  multiple-incoming-connector and cyclic topology restrictions while preserving
  exact register-unit aliases and target custody.

  Acceptance: pressure-driven split points, copies and segment homes replay
  independently through callable publication on each admitted target.
  Moved/omitted splits, incompatible fixed-use crossings, live-unit aliasing and
  stale analyses reject. SPILL-REALIZATION owns storage; this item adds no slot.

- **FRAME-LAYOUT.** Realize bounded activation claims under
  [activation storage](wiki/spec/resources/activation_storage.md).
  The contract is ratified. TASKS.md's **RUNTIME-SIZED-ACTIVATION-CONTRACT**
  owns the missing source/Terminal claim route; ordinary general-call frames
  do not depend on runtime-sized claims. Existing red-zone, probing, unwind,
  callee-save and call-alignment plans are not missing mechanisms.

  Wave evidence (macw5): the upstream dependency is deeper than a missing
  input. No claim roster reaches native lowering — no `ActivationClaim`/
  claim-site field exists on `PostAllocationMachineFunction`, which is
  wire-versioned, so the durable roster shape is upstream's decision. The
  realization model is also unpinned: co-live claims must pack disjointly
  while exclusive claims share storage, a tree-packing the spec does not
  define, and whether committed extent equals the composed charge or a
  larger physical extent is unspecified. `extents::activation_claims::
  compose_claim_bounds` already implements the aligned-sum/maximum
  composition rule and is directly reusable once the contract pins the
  roster transport, coordinate assignment and extent semantics.

  Once exact claims reach native lowering, compose simultaneously live bounds
  with alignment and mutually exclusive bounds by maximum. Retain committed
  extent computation, activation provenance, release order and suspension
  custody, joined to the final frame/probe/unwind plans. Reuse permitted eager
  frame commitment or lazy claim-site commitment; neither grants unbounded
  stack allocation.

  Acceptance: per-site replay checks committed extent within its bound and
  provisioned activation, site/plan bijection, release and suspension custody.
  Missing/duplicate/invented claims, suppressed bounds and incorrect ordering
  reject. Establishment failure remains checked, not a trap or clamp; run
  native claim access/release on each admitted matching target.

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
  overlap. Byte-view payload roster rows now bind the resolved backing storage
  root at the producer (`ByteViewHomes`), so subslice-view accesses charge the
  root instead of mislabeling the view identity; extend that premise binding to
  retained fact identities beyond storage roots. SelectedMemoryAccess and
  rewrite receipts still do not carry the loan/compatibility/fact identities
  used to justify the general premise. Exclusive ownership alone does not
  establish projection disjointness; follow
  [loan semantics](wiki/spec/terminal-psi/loans.md), not differing PlaceIds.

  Preserve shared storage-route and extent semantics in `place_storage` across
  the rewrites and commutation checking. Remaining control-flow work includes
  forwarding where incoming stored registers differ and store motion across
  non-reconvergent forks/joins without adding or dropping writes (forks whose
  edges reconverge on one join fed only by the crossed block and
  sole-predecessor arms now cross, derived independently by admission and
  validation). Unsupported dynamic geometry
  must reject until justified; do not equate storage routes or reuse stale
  evidence. Existing off-range constant-index, Structural-slot, and same-extent
  CopyBytes cases are controls, not new implementation tasks.

  Acceptance: source-produced exact selections execute natively and replay
  after publication; empty/disabled selections retain identity output. Preserve
  reads through shared reborrows or field projections, intervening calls/effects,
  volatile/placed accesses, partial overlap, stale candidates, exhausted budgets,
  and corrupted fact-identity negatives. Source admission and the independent
  validator must each establish the required alias and byte-extent facts.
- **REPRESENTATION-SPECIALIZATION.** Extend field/variant relevance beyond
  existing proven membership and scalar-constant observation folds in
  `abstract-operations-to-abstract-operations/src/{representation_specialization,field_value_specialization}/`.
  The next field-value mechanism is substitution of a proven nonconstant
  initializer at its uses, preserving exact place/path/provenance and effects.

  Invariant-window specialization depends on an upstream operation/evidence
  contract retained into this stage; no such operation reaches it yet. Do not
  invent that input locally. Preserve the authenticated cyclic freeze unless a
  checked transformation reconstructs the affected evidence.

  Acceptance: exact-selected source-produced specialization publishes and
  independently replays. Stale/forged field/path/value evidence rejects,
  disabled selection is identity, and unsupported paths, custody and cyclic
  changes remain rejected under
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage).
- **INTERPROCEDURAL-SUMMARIES.** Add proof-bound inlining and the summaries/
  substitution it needs in `abstract-operations-to-abstract-operations`.
  Existing `analyses/` computes transitive effects and a direct call graph,
  not exact callee place reads/writes or an inlining transformation.
  Extend summaries with argument/result and place-access relationships; a
  coarse structural-state effect is not non-aliasing evidence.

  Acceptance: an exact-selected source-produced call rewrite independently
  reconstructs substitution, ownership, effects, cleanup, proof/provenance and
  resource correspondence through publication. Stale summaries and incompatible
  aliases reject; disabled selection preserves ordinary execution. Keep
  elaboration/proposal separate from the independent validator.

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

- **BENCHMARKS.** Complete missing subject/selection/target measurements
  through [tools/benchmark](tools/benchmark/README.md), using the existing
  record schema and generated [benchmark matrix](wiki/drafts/measurements/benchmarks.md).
  Linux native and enabled/disabled selection rows, cross-target compile rows,
  a measured macOS ARM64 selected row, and a compile-only structural-proofs
  row already exist; do not rebuild the harness or repeat first-row work.

  Remaining host acceptance: matching-host Linux ARM64, Windows x64 and Intel
  macOS measurements, plus an applicable UEFI subject and QEMU/hardware route.
  Windows job-object peak-memory collection is implemented; exercise it on
  Windows and report unavailable metrics honestly. The harness's Intel macOS
  `HOST_LEGS` label still says native realization is pending; reconcile that
  stale label with **MACOS-X64-HOST-PROFILE** in [TASKS.md](TASKS.md), whose
  remaining acceptance is matching-host execution, not a new backend.

  Commit missing `prime_counter` (`--expected-exit 8`) and `standalone`
  measurements after reproducing their current package/entry acceptance.
  Historical remainder and `Filesystem::host` failures are not current
  blockers without reproduction. The proof-call selection repair is present;
  `math_proofs` still needs an authored entry/product choice before native
  measurement. Preserve compile-only proof work as such rather than inventing
  meaningful runtime behavior for an inert entry. A dependency-free subject
  is not inherently restricted to `--no-run`.

  Acceptance: use the existing prepare/measure/validate/matrix flow; records
  bind exact subject/compiler revision, authored enabled/disabled selection,
  admissions, target, measuring host and observed exit behavior. Report
  compile time, peak memory, code size and runtime with explicit skipped,
  unavailable or non-applicable reasons, and regenerate the matrix.
  `wrapping_square_sum` now enables six Psi rules, so its authored row is a
  selected row, not the historical empty-default cell. A non-applicable
  subject/target pairing must not erase the still-unmeasured host leg or
  disguise an unrelated compile/review failure. Keep records descriptive of
  the measurement; no per-cell task proliferation or session history.
