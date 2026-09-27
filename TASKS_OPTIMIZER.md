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

## Execution priority

Keep the full scope below. Prioritize by the unchanged program or required
publication that the work enables; a helper milestone does not close that
customer. Source and call/storage lowering blockers stay with
[TASKS.md's immediate product work](TASKS.md#immediate-product-closure).

| Order | Work to take first |
| --- | --- |
| 1. Correct native execution | GENERAL-CYCLIC-EXECUTION and SPILL-REALIZATION when a real program needs their call/storage route; TRANSLATION-VALIDATION where an admitted operation cannot publish required evidence. Carry the applicable PER-RULE-COVERAGE checks with each repair. |
| 2. Better code for runnable programs | JOIN-INVARIANT-REWRITE-TRANSPORT, GENERAL-LICM, ALLOCATION-REFINEMENT, machine/memory rewrites, representation specialization and interprocedural summaries. FRAME-LAYOUT follows its upstream runtime-sized activation claim route; ordinary calls do not wait for it. |
| 3. Consolidation and broader rollout | PIPELINE-OWNER-CONSOLIDATION and CUSTODY-MUTATION-COVERAGE where they remove duplicated work or protect the repaired route; BENCHMARKS and WORKSPACE-ROLLOUT retain their full host/evidence and promotion requirements. |

A concrete correctness or validation regression takes precedence over this order.
Mandatory pressure recovery is not optional optimization quality; advanced
rewrites and promotion are not prerequisites for the first useful compiler.

## Validation, translation, and publication

- **TRANSLATION-VALIDATION.** Complete source-to-physical occurrence coverage
  on the common native route under
  [closed application and physical occurrence](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence).
  Owners: stage 07's `boundary_operator_custody/` for checked-to-Terminal
  occurrence replay, and `native-artifact/src/physical/` for emitted spans.

  Follow the missing producer/consumer join, not the physical match table
  alone. `checked_boundary_operator_occurrences` currently reconstructs float
  and integer comparisons. Stage 05's maintained
  `tests/boundary_operator_frontier.rs` pins named operator initializers at
  flow-call planning and spelled operator initializers at pure-initializer
  admission. Reproduce each source customer at current main; repair earlier
  admission with its source owner, retain exact checked demand and Terminal
  occurrences, then extend physical replay. A `BoundaryCall` belongs to the
  boundary-settlement lane, not the operator-call lane.

  `derive_operator_physical_span` handles checked static calls, comparison
  spans and FMA; other intrinsic families still fall through to a named
  `UnsupportedOperatorSpan`. Preserve their full scope, including float
  arithmetic, negation, conversion and builtins, without claiming their old
  source refusals are current evidence. Dynamic coverage has a different join:
  stage 07 reconstructs local rebound scalar/unit descriptor-table calls,
  while forwarded/register-indirect occurrence production remains open.
  `derive_dynamic_call_span` already consumes rebound, stored, parameter and
  forwarded emission records with multiple relocation windows; do not rebuild
  those mechanisms as the missing upstream occurrence route.

  Acceptance: source-produced surviving occurrences each bind exactly one
  physical child with nonempty machine/object/final-image spans, independently
  replayed through publication. Preserve verified-elimination exemptions and
  port-effect settlement ownership. Missing, duplicate, stale, substituted,
  padded and role-swapped children reject; unsupported coverage identifies its
  exact occurrence. `physical/derivation/tests.rs` covers coordinate bijection,
  dispatch roles and gap identities with constructed fixtures; it does not
  replace compiler-produced publication evidence. General calls do not wait
  for FRAME-LAYOUT claims. Extend ordinary operations and receiving checks,
  without restoring retired whole-function planners.

- **CUSTODY-MUTATION-COVERAGE.** Migrate the remaining hand-written custody
  substitution matrices to `psi/foundation/mutation-matrix`, preserving each
  family's independent checker and existing corruption cases. Use
  `custody_field_inventory!` and `run_one_field_substitution_matrix`; the
  `optimization-core` re-export is for Omega consumers, not a Psi dependency.

  Start with the manual matrices in Terminal codec
  `tests/artifact/{proposition_vocabulary_custody,reborrow_restored_call_use_custody,trace_profile_custody}.rs`,
  native-artifact's semantic-wrapper container/receipt tests, image region and
  footprint tests, image-emission's internal scalar-call/boundary-settlement
  tests, and provider-evidence's `component_era_entry_ledger/custody.rs`.
  Recheck surviving selected-dispatch source-edit matrices and any custody
  families formerly owned by compiler tests before scheduling a migration:
  deleted test targets do not establish equivalent coverage.

  The codec's float-meaning, proof-section, recursive-component and root-handoff
  matrices already use the shared driver, as do build-output, package review
  evidence, component-description and external-root admission. Use their actual
  checker closures as examples; a test name alone does not identify a manual
  matrix. Component-publication's runnable-component test covers lifecycle joins
  and must retain that behavior.

  Acceptance: each representable field changes independently, containing
  identities recompute honestly, and the family's independent replay rejects
  substituted custody. Name unrepresentable fields and retain their canonical
  encoding/decoding refusals. Audit missing inventories and inventory-to-driver
  coverage explicitly, including inventories without matrix/driver consumers.
  New families retain the [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage)
  axes; shared fixtures must not replace independent replay with producer admission.

## Register allocation and frames

- **SPILL-REALIZATION.** Finish executable register-pressure recovery and
  prove that its emitted frames participate in stack provisioning. Owners:
  [register allocation](omega-rust/omega/pipeline/05_selected-instructions-to-register-homes/README.md)'s
  `assignment/runtime_spill/` and selected-instruction
  `rewrites/runtime_spill/`; the latter independently replays stores, reloads,
  physical geometry and value lineage.

  Remaining work:

  - Admission: foreign-class values without a pure bit-transport pair (including
    vector/non-IEEE values) and entry-bound victims with an edge back to the
    entry block still reject in `runtime_spill/admission.rs`. Preserve that
    refusal until storage geometry and entry redefinition replay are supported.
    IEEE transport and early-clobber tied uses with shared co-readers already
    work; `tests/read_modify_write.rs` pins a dedicated reload for the tied use.
  - Composition: fixed-view probe declines, pressure after materialized copies,
    and pressure after active-resident rematerialization already enter runtime
    spill with their validated prefix (`assignment/recovery.rs`). Completed
    literal-fold/pre-allocation runs still take `assignment::transformed` homes
    directly in `stage_register_allocation`; connect residual pressure there
    without losing rewrite custody. Close remaining multi-recovery selection
    composition through the same route and independent replay.
  - Slot reuse: emitted recovery uses eight-byte `LocalStorageSlotId::Spill`
    homes. `rewrites/runtime_spill/slot.rs` reuses a slot only when last-writer
    replay proves noninterleaving windows for its supported store/load forms.
    Generalize interval-based reuse on this physical route. The separate
    `assignment/stack_slot_coloring` and logical-spill boundaries have no
    sequenced runtime caller; do not mistake their validated plans for physical
    input or restore the removed logical-plan join. Retire superseded
    `unsequenced_spill_stages/` owners with their consumers instead of extending
    parallel slot-assignment implementations.
  - Stack provisioning: carry a witnessed runtime-spilled function through
    image-emission's final-frame production and `function_fragments/validation/stack.rs`,
    `object_artifact/stack_demand.rs` and
    `installation_record/record_construction.rs::derive_installation_stack_demand`,
    then external-root admission or hosted-receiver partitioning. Reuse those
    existing demand/supply joins under the
    [compiler-owned stack contract](wiki/spec/resources/storage.md#compiler-owned-stack-accesses);
    ABI entry-spill fixtures alone do not establish allocation-recovery coverage.
    Target-required setup and probing belong to frame/provisioning owners.

  Acceptance: a compiler-produced pressure customer executes through the common
  native route with independently replayed spill operations and final-frame
  demand. Slot reuse is counted once; changed allocation/frame realization
  invalidates stale demand; insufficient supply rejects before execution.
  A byte ceiling is not valid stack backing. Preserve the write-only hosted
  control (`python3 tools/corpus_gate.py --native --filter borrow/write_only`)
  and do not introduce source crash routes for private spill operations.

- **ALLOCATION-REFINEMENT.** Add pressure-chosen live-range splits that
  connect independently homed segments before private storage when a copy can
  free a home. Stage 04 owns
  `analyses/fixed_precolored_split_requirements/{compute,replay}/` and
  `rewrites/allocation_recovery/fixed_view_copy/`; stage 05's
  [allocation recovery](omega-rust/omega/pipeline/05_selected-instructions-to-register-homes/src/assignment/runtime_spill/recovery.rs)
  owns the pressure decision and subsequent assignment.

  Existing cuts authenticate fixed uses; fixed-view copies already cover
  shared source exits. `split_domain_pressure` and its candidate tests score
  pressure across existing tied/edge-transfer domains, without inserting new
  split copies. Runtime recovery tries rematerialization and spilling; it
  does not implement arbitrary pressure-chosen splits. Recompute liveness,
  ranges and legality after changing the selected graph. Extend the fixed-split
  path's tied-register, early-clobber, multiple-incoming-connector and cyclic
  restrictions while preserving exact register-unit aliases and target custody.

  Acceptance: a compiler-produced pressure customer demonstrates useful splits
  and copies before unnecessary storage. Split points and segment homes replay
  independently through callable publication on each admitted target.
  Moved/omitted splits, incompatible fixed-use crossings, live-unit aliasing and
  stale analyses reject. SPILL-REALIZATION owns storage; this item adds no slot.

- **FRAME-LAYOUT.** Realize bounded activation claims under the ratified
  [activation storage contract](wiki/spec/resources/activation_storage.md).
  TASKS.md's **RUNTIME-SIZED-ACTIVATION-CONTRACT** owns source checking and
  Terminal claim production. Ordinary general-call frames do not wait for
  this feature; red-zone, probing, unwind, callee-save and call-alignment
  mechanisms already have owners.

  No claim roster reaches `PostAllocationMachineFunction`. Coordinate its
  durable transport with the upstream producer, then join claim sites to
  [native frame realization](omega-rust/omega/backend/machine-emission/src/function_realization/frame.rs).
  `extents::activation_claims::compose_claim_bounds` computes aligned sums and
  branch maxima over supplied rows; its ledger tests do not establish compiler
  transport, physical coordinates or probe coverage. Implement placement and
  padding with disjoint co-live claims and reuse for mutually exclusive claims.
  Bind demand to the final validated physical extent under
  [stack demand and backing](wiki/spec/resources/storage.md#stack-demand-and-backing);
  a numeric composition alone is not a layout proof. These are implementation
  obligations under the existing contract, not pending language decisions.

  Charge authored bounds, never runtime committed values. Retain each site's
  committed-extent computation, activation provenance, stable addresses,
  reverse release order and suspension custody in the final frame/probe/unwind
  plans. The contract permits eager frame commitment or lazy claim-site
  commitment with per-granule probing; neither permits unbounded allocation.

  Acceptance: independent per-site replay checks committed extent within its
  bound and provisioned activation, site/plan bijection, release and suspension
  custody. Missing/duplicate/invented claims, suppressed bounds and incorrect
  ordering reject. Establishment failure remains checked, not a trap or clamp;
  run native claim access/release on each admitted matching target.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Complete the receiving/native half of
  [ranked callees on projected receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers).
  The same-name TASKS.md item owns source production and verifier coverage.
  Follow the ordinary [native admission route](omega-rust/omega/pipeline/00_terminal-psi-to-abstract-operations/README.md#ranked-native-admission):
  natural-ranked and unranked graphs retain their custody, and authenticated
  cyclic components remain frozen under optimization. No countdown-only fallback.

  Make `termination/ranked_callee_projected_receiver_compile` an actual native
  witness before treating its corpus outcome as execution evidence. Its source
  calls a ranked callee through a projected receiver and observes the write,
  but the fixture has no `build.omg` and is neither a run-tier nor an `_exit`
  case. `corpus_runner` therefore only checks it even with `--native`; the
  macOS native golden records `checked`. Supply the intended host ProgramEntry
  bindings and an `expected_native_exit.txt` oracle, then use
  `python3 tools/corpus_gate.py --native --filter termination/ranked_callee_projected_receiver`.
  Ensure the receiver starts nonzero so the exit-70 observation proves the
  callee's write, rather than merely observing its zero-initialized value.

  Acceptance: execute the composed call on both Linux architectures; verify
  exact projection, referent and argument identity, callee ranking, return,
  cleanup and composed resources. Conflicting parent access and missing/invalid
  required ranking reject. Iterations add no frames; the outer call may have
  one. Changing-reference transfers additionally need represented loans/aliasing
  and ranking substitution; a fixed-referent witness or wider parameter count
  does not close them. Keep earlier source/lowering refusals with their owning
  task while carrying this customer through native replay.

- **JOIN-INVARIANT-REWRITE-TRANSPORT.** (new-scope) Preserve useful scalar
  rewrites when retained join invariants name the values being rewritten, or
  expose a deliberate refusal through an observable optimization decision.
  Owners: stage 05's `proofs/scalar_block_invariants/joins.rs`, stage 06's
  `copy_propagation` and `dead_scalar_elimination`, and their independent
  Terminal rewrite validators.

  Literal joins no longer create invariants in machines with no reconstructed
  obligation: `joins::candidates` gates the interval proposal on a machine-owned
  question. The remaining case is a proof-bearing machine. In
  `preterminal_optimization.rs`,
  `consumed_literal_join_keeps_the_invariant_and_refuses_rewrites` combines a
  runtime-index obligation with constant-return arms and asserts both passes
  leave the complete module unchanged. The passes return the original module
  on a proof-question-changing proposal; their execution records retain identity
  output but do not explain that refusal. This preserves correctness and is
  an optimization limitation, not a blocker to ordinary native execution.

  Acceptance: a source/corpus customer with a consumed join invariant either
  retains the selected rewrites through independently checked proof-context
  transport, or reports the deliberate refusal. Cover the no-obligation control
  and both passes. Preserve exact proof questions, value/ranking identities and
  rejection of forged context changes; changing arm constants to avoid the
  invariant does not test the missing behavior.

- **GENERAL-LICM.** Extend invariant motion beyond the current operation
  and custody restrictions and authenticated shared-source preheader. Owner:
  `01_abstract-operations-to-abstract-operations/src/ranked_rewrites/loop_invariant_scalar_motion/`,
  with admission in `src/validation/invariant_calls.rs`,
  `place_observations.rs` and `invariant_operations.rs`, and replay in
  `context/ranked_cycles/freeze/relocated_scalars.rs`.

  `PsiOptimization::LoopInvariantScalarMotion` already schedules proposal,
  validation, application and publication through `optimize_abstract_operations`.
  `tests/scheduled_step.rs` verifies retained commits, manifests and ledger
  replay; it stops at optimizer publication, not native execution. Extend this
  production route and carry PER-RULE-COVERAGE with each new transformation.

  Remaining boundaries:
  - Remaining establishment/custody families, including affine/linear Owned
    argument transfers, claim transfers, and structural-result calls carrying
    requirement obligations or selected evidence. Copyable unrestricted
    whole-root arguments and admitted claim-free affine/unrestricted results
    already move. `Call`, `CallUnit` and `CallStructuralScalar` already carry
    discharged requirement obligations; do not treat all obligated calls as
    unsupported. New cyclic source operations must pass ordinary Psi verification.
  - Structural predicates in crash-continuation rosters. All four call forms
    already re-derive supported rosters from verifier-owned callee contracts;
    structural-signature calls still require scalar-only predicates because
    canonical structural-path substitution is missing. Preserve
    `tests/calls.rs::structural_predicate_crash_roster_stays_inside` until that
    evidence can be reconstructed, along with pure-callee, argument-invariance
    and non-speculation controls.
  - Profitability beyond the current negative relocated-node-count estimate
    and baseline selector. Keep semantic non-speculation, logical fuel and
    optimization cost distinct under
    [optimization semantics](wiki/spec/build/optimizations.md).
  - Motion needing new blocks or run duplication, with corresponding
    block/occurrence evidence, rather than only the existing preheader.

  Preserve invariant operand substitution and topology-based non-speculation.
  `StructuralByteSequenceFieldRead` additionally requires same-root/path/field
  length evidence and proposition-pinned index/length identities: substituting
  invariant member parameters is insufficient, though producers in the same
  relocation run can qualify. Acceptance independently reconstructs components,
  loop-carried custody, ranking, provenance, effects and fuel. Forged operands,
  retained internal discards, missing exit disposals and stale frontiers reject.
  Rebuild ownership membership after transformation and preserve Terminal-derived
  region custody; do not add a private loop forest or restore countdown-specific
  authority.

## Machine optimization

- **EXACT-MACHINE-SIMPLIFICATIONS.** Finish source-produced execution of
  retained copy, extension, address, compare/test and scheduling rewrites in
  [selected-stage optimization](omega-rust/omega/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs).
  `PRE_ALLOCATION_RULE_CATALOG` already executes eight exact rules through
  `optimize_selected_instructions`: same-block copy, redundant extension,
  address offset, constant boolean/branch, and dead/redundant/equivalent compare.
  Keep their joint fixed-point sweep and `PreAllocationPolicy`; adding their
  existing names or wiring compare removal again is not remaining work.

  Close the actual producer/publication gaps first. The public-entrance
  `evidence_matrix/pre_allocation.rs` starts from constructed Terminal modules
  and stops at selected-stage replay. Its address-fold case asserts no commit;
  real byte-field reads feed `AddressOffset` into `Load8Indexed`, outside the
  rule's admitted consumer table. Connect useful source-produced address
  windows instead of treating that negative fixture as positive coverage.
  Dead-compare removal needs the constant-boolean/branch composition that
  removes flag readers; solo-selection refusal is not a missing executor.
  The `optimizer/*compare_selection_compile` and
  `constant_branch_selection_compile` corpus fixtures have host entries but no
  run oracle, and the native golden records only `built`. Extend the relevant
  witnesses to observe results and establish that the selected rule committed,
  then replay the published artifact. Building a selected program alone proves
  neither application nor execution.

  [Staged rewrites](omega-rust/omega/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs)
  still have no catalog/execution join for boundary boolean/branch folds or
  scheduling. Reuse the existing `relocation` member-run/destination mechanism
  and `scheduled_relocation` dominance/dead-path admission; geometry-specific
  copies have already been consolidated. Preserve the distinct arm, inflow,
  confluence and memory-commuting premises, and `interchange`'s two-run swap.
  Do not force commutation through the ordinary MemoryOrdering refusal or
  lose fork admission when skipped-arm traversal consumes the path budget.
  Cross-block copy substitution and remaining compare/test vocabulary stay in
  scope. Preserve x86-64 displacement bounds and AArch64 scaling constraints.

  Bind new candidates to current source and exact selection; retain ordered
  catalog, rule/validator, policy, analysis/invalidation, budget and applicability
  identities through allocation and publication under
  [independent replay](omega-rust/optimization.md#catalogs-and-independent-replay).
  Reconstruct legality independently: redundant/equivalent compare admission
  and validation still share `condition_state::reaching_events`; an independent
  operand-stability check does not independently establish that reaching set.
  Restore-by-content alone only checks the edit. DECLARATIVE-PEEPHOLES owns
  descriptor generalization and ordinary literal nomination; ALIAS-AWARE-MEMORY
  owns memory rewrites.

  Acceptance: source-produced programs select each retained rule by exact name,
  demonstrate a real commit, run on a supported host and replay after publication.
  Empty/disabled selections reproduce identity. Include a valid window beyond
  existing shape families, register/condition hazards, crossed calls/effects,
  noncommuting memory, boundary settlements, stale candidates, exhausted budgets,
  and a producer legality error refused by independent validation. Keep required
  compositions explicit; hand-built helper plans do not close this customer.

- **DECLARATIVE-PEEPHOLES.** Execute declarative relationships for physical
  register units, effects, traps, memory, stack and control flow through the
  selected-stage catalog, and nominate ordinary literal folds beyond pressure
  recovery. Owners: `rewrites/selected_lowering/literal_fold/` and
  [staged peepholes](omega-rust/omega/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/peepholes/mod.rs).

  The literal route still takes one incoming
  `ImmediateU64RematerializationCandidate` per function/iteration under
  `frameless_leaf_caller_saved_views`. Its sole consumer may now occur later in
  the same block; adjacency is no longer required. Generalize nomination to
  non-pressure materializations without weakening use/provenance custody.
  `COMPARE_ZERO`/`COMPARE_LEFT_ZERO` already select `CompareI64Zero`; the
  left-zero test covers equality readers and rejects ordering readers on both
  architectures. Preserve that behavior through the common descriptor route;
  do not restore a separate literal producer.

  Several relationships exist as descriptors and helper-tested rewrites but
  are still unexecuted:

  - `terminator_pair` and `condition_materialization` retire decided flag uses,
    preserve the required unit traffic, and rewrite branch control or a boolean
    result. They overlap the executed constant-branch/boolean families; integrate
    at the existing semantic owner rather than creating another enablement path.
  - `projected_access` carries pointer reads/writes and preserves architectural
    faults while absorbing an address displacement. Reconcile it with the
    executed address-fold family, including its target-specific bounds; the
    staged descriptor's common scaled-immediate bound is narrower on x86-64.
  - `copied_call_operand` retains ABI pins, implicit uses/definitions, clobbers,
    trap/control effects and the target's call-stack lifecycle while rebinding
    copied arguments. Its helper coverage does not establish native execution.

  Complete catalog discovery, application and publication for those dimensions
  before claiming them closed. Tied operands and further hosted-trap relationships
  remain unsupported. Keep independent axes for operand shape, unit roles,
  fault discharge/preservation and other machine effects; do not multiply
  variants for combinations or add another identity over already-covered axes
  as a substitute for the missing relationships.

  Validator independence remains work: the staged flag proposers and replay
  both call `condition_flow::reaching_event`, the algorithm that chooses the
  reaching producer. Reconstruct that fact independently, as required by
  [catalogs and independent replay](omega-rust/optimization.md#catalogs-and-independent-replay);
  avoiding the descriptor table is insufficient. Preserve literal replay's
  instruction-derived grammar and separate repeated `SourceShape` position
  mechanics from identity-specific mathematics without reading producer
  descriptors or trusting producer legality.

  Acceptance: every required dimension has an explicit selection and a firing
  rule on x86-64 and AArch64, independently reconstructed relationships, and
  compiler-generated publication replay. Include corruption, wrong-policy,
  boundary and budget negatives under
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage),
  including a wrong producer admission that replay rejects. Relationships with
  no independent replay stay rejected; helper-only declarations do not close
  the execution requirement.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Connect load forwarding, dead-store elimination
  and store motion to exact selected-stage execution, allocation and publication.
  The existing implementations and tests live under
  [rewrites/unexecuted/](omega-rust/omega/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs),
  not the executable catalog. Reuse their proposers and independent legality
  validators; EXACT-MACHINE-SIMPLIFICATIONS owns the common stage-execution
  join, not a second memory-specific pipeline.

  Establish the alias premise each admitted window needs. `commuting_accesses`
  currently treats different `StorageRoute::Place` identities as disjoint;
  the memory rewrites likewise rely on the access roster's root/extent claims.
  Bind general distinct-place reasoning to retained loan/compatibility/fact
  identities, or independently prove non-overlap at the roster producer under
  [loan semantics](wiki/spec/terminal-psi/loans.md). Exclusive custody alone
  does not prove projection disjointness. `ownership_frontier_facts` reach
  `LegalizedScalarFunction` and its identity, but selected access rows and
  rewrite receipts still do not bind the general compatibility premise.

  Preserve the existing backing-storage route: `ByteViewHomes` and
  `view_backing_roots` charge subslices and bound view parameters to their
  possible backing roots. Independently check that reach and carry any further
  premise identities to the consumer; different view IDs are not disjoint bytes.
  Keep `place_storage` as the shared route/extent vocabulary, distinguishing
  producer-owned storage from temporary staging. Existing constant-index,
  Structural-slot and same-extent CopyBytes cases are controls.

  Narrow the remaining control-flow work to what the helpers cannot express.
  Forwarding already merges different incoming stored registers through an
  existing block parameter whose edge bindings match every leg; the propagated
  merge test also replays it. A join with no lawful existing merge remains open.
  Store motion already crosses nested reconvergent regions that preserve one
  write on every traversal. Non-reconvergent forks/joins, external predecessors
  and unsupported dynamic geometry still need justification without adding or
  dropping writes. Preserve safe refusal until that evidence exists.

  Acceptance: source-produced exact selections actually commit, execute natively
  and replay after publication; empty/disabled selections retain identity.
  Cover shared reborrows and field projections, intervening calls/effects,
  volatile/placed accesses, partial overlap, stale candidates, exhausted budgets
  and corrupted premise identities. Retain the helper tests that feed forged
  edits directly to validation, including unaccounted writes and partial covers.
  Producer admission and independent replay must each establish aliasing,
  byte extents, register transport and write multiplicity on the admitted path.

- **REPRESENTATION-SPECIALIZATION.** Extend field/variant relevance under
  `01_abstract-operations-to-abstract-operations/src/{representation_specialization,field_value_specialization}/`,
  with independent candidate replay in `optimization-unit-semantics`.
  Proven membership, constant fields, nonconstant initializer forwarding,
  stored-whole child descent and uniform block bindings already have source
  controls; the pass matrix publishes forwarding and retires the read.

  Extend the evidence basis beyond those cases. Type-path descent through
  fields, indexes and referents is not a proof of stored contents:
  establishment descent crosses exact owned children, while scalar-array
  elements are not separately placed children. A call-produced child's contents
  remain unproven (`nested_unestablished_child_stays_unproven`);
  INTERPROCEDURAL-SUMMARIES owns callee result evidence. Reprobe runtime-index
  membership and integer field reads beneath fixed indexes from source before
  assigning a blocker. Record admission now includes sum/mixed children and
  scalar arrays; the old record-only restriction is obsolete.
  `EstablishStructuralCase` still rejects in stage 00's operation routing,
  so that customer needs ordinary operation admission before specialization.

  Borrowed restoration windows already reach abstract operations as
  `MoveStructuralField`/`StoreStructuralField`; uniform-binding proofs reject
  rewritten or mutably re-lent places. Window specialization must reconstruct
  the exact open/restore custody and affected observations. Any additional
  window family needs its upstream operation/evidence contract; do not invent
  that input in the optimizer. Preserve authenticated cyclic freeze until the
  affected custody and ranking can be reconstructed.

  Acceptance: exact-selected source-produced specialization commits, publishes
  and independently replays; carry PER-RULE-COVERAGE through native execution.
  Stale/forged field/path/value evidence rejects, disabled selection is identity,
  and unsupported paths, custody and cyclic changes remain rejected under
  [stage-extension validation](omega-rust/optimization.md#validation-when-extending-a-stage).

- **INTERPROCEDURAL-SUMMARIES.** Add proof-bound inlining and exact
  argument/result/place-access summaries in abstract-operations stage 01.
  Reuse `analyses/semantic/effect_summaries.rs`'s revision-bound transitive
  effects and `analyses/control_flow/call_graph.rs`'s call components. The graph
  already includes known dynamic realizations and nominal cleanup calls;
  neither product gives precise callee reads/writes or an inlining transform.
  A coarse structural-state effect is not non-aliasing evidence.

  Bind summaries to callee revisions, projected storage and aliases. Integrate
  inlining through an exact selection, analysis invalidation and independent
  candidate replay under [optimization semantics](wiki/spec/build/optimizations.md).
  Constructed analysis fixtures do not establish a source-produced call rewrite.

  Acceptance: the rewrite independently reconstructs substitution, ownership,
  effects, cleanup, proof/provenance, fuel and resource correspondence through
  publication. Stale summaries and incompatible aliases reject; disabled
  selection preserves ordinary execution. Keep proposal and validation separate,
  and carry the rewritten source customer through PER-RULE-COVERAGE.

## Pipeline cleanup follow-ups

These are separately schedulable follow-ups, not an ongoing cleanup run.
The [ownership contract](omega-rust/pipeline.md)
governs these tasks: replace obstructive implementations, preserve semantics and
independent validation, and keep empty/nonempty optimization selections on one
physical route. Unsupported cases reject rather than restoring a fallback.

- **PIPELINE-OWNER-CONSOLIDATION.** Finish the public-entrance and consumer
  audit under the [connected pipeline contract](omega-rust/pipeline.md).
  Trace qualified identities through repository-wide consumers, then remove
  competing entrances or orphan outputs and make retained plans constrain the
  physical operations they describe. A common name such as `encode`, an
  isolated validator, or a retained-but-unused plan does not establish a join.

  The common physical route already runs through
  `native_pipeline/physical_pipeline` and machine-emission's fixed-frame
  realization. Native-artifact owns receiver admission and
  `publish_fragment_object`'s wrapper/entry/evidence joins; machine-emission
  owns private callback materialization. Preserve these owners and independent
  representations while keeping compiler coordination limited to typed products.

  Selected catalog execution remains with EXACT-MACHINE-SIMPLIFICATIONS,
  ALIAS-AWARE-MEMORY and DECLARATIVE-PEEPHOLES. SPILL-REALIZATION owns recovery,
  final-frame provisioning and retirement of superseded spill boundaries with
  their consumers. UEFI/provider/ABI and callback capability gaps remain with
  their native owners; source/Build orchestration is TASKS.md's pipeline route.

  Acceptance: no competing entrances or orphan products; empty/nonempty
  selections share physical execution and independent publication. Preserve
  image-emission's source-produced fragment fixture and native-artifact's
  detached-plan/object rejections. Exercise affected routes through publication;
  prefix consumers may use the same stage functions without another driver.

## Verification and rollout

- **PER-RULE-COVERAGE.** Complete each executable exact rule's exact-selection, positive,
  negative, boundary, disabled/empty identity, rollback, composition, budget,
  determinism, fixed-point, corruption and downstream-publication coverage
  through its public stage entrance. Follow the
  [stage-extension matrix](omega-rust/optimization.md#validation-when-extending-a-stage)
  as EXACT-MACHINE-SIMPLIFICATIONS and ALIAS-AWARE-MEMORY connect more rules.
  Reuse existing stage-06 tests and stage-01 `pass_manager/tests/evidence_matrix`
  support; their hand-built carriers establish stage behavior, not source-to-native
  coverage. Keep [inventory](omega-rust/omega/foundation/optimization-core/rules.md)
  entries tied to current owning catalogs and tests; the former inventory gate
  does not certify missing axes or an unsupported native composition.

  Restore actual rollback-to-identity coverage. The corpus fixture
  `optimizer/rollback_to_no_selection_empty_entry` enables ControlFlowCleanup,
  but `corpus_runner::build_native_fixture` constructs a request with no rollback
  overlay: running that fixture does not test its claimed disabled-rule rejoin.
  Existing CLI negatives cover invalid names and product restrictions. Exercise
  an authored selection with the real disable overlay, verify its requested/
  applied/effective receipt, and compare publication with the ordinary empty
  selection apart from explicit rollback provenance.

  Acceptance: each applicable axis executes and independently replays through
  the rule's public entrance and compiler-produced publication. A fixed-point
  check feeds the published artifact back as legal input; repeated construction
  proves only determinism. Explain non-applicable axes, including mandatory
  recovery's lack of a disabled mode. Share fixtures without losing distinct
  corruption or boundary cases; a green corpus observation alone is not proof
  that a rule fired or that a rollback was exercised.

- **BENCHMARKS.** Complete trustworthy subject/selection/target measurements
  through the existing [prepare/measure/validate/matrix workflow](tools/benchmark/README.md)
  and [benchmark matrix](wiki/drafts/measurements/benchmarks.md). Existing records
  include Linux x86-64 and macOS ARM64 execution, selected/disabled rows and
  cross-target compile legs; they are evidence for their recorded revisions,
  not a current compiler baseline. Do not rebuild the harness.

  Repair evidence gaps in `tools/benchmark/benchmark.py` before using new rows
  for promotion:

  - Bind the record to the actual compiler build and effective selection.
    `measure` records checkout HEAD and a binary path, not the binary's build
    identity; `--enable` only augments the row label, while compiler arguments
    carry only `--disable-optimization`. Use authored selections and reconcile
    the compiler's receipt instead of claiming a metadata-only enable executed.
  - Require the expected runtime result for acceptance: `measure` can publish
    `exit_code_match: false`, and `validate_record` accepts that record. Preserve
    failed observations as failures, not successful benchmark evidence.
  - Settle the requested target's package review. `settle_package_review` takes
    the first `review:` line, which may name another target's already-accepted
    document and skip the needed resume. Reproduce dependent subjects through
    current preparation and route its actual compiler failure to its owner;
    old std carrier/provider diagnostics are not a standing migration task.
  - Keep matching-host gaps visible. `matrix_rows` suppresses a target's pending
    host row after any applicable record, even when every runtime leg is a
    cross-compile skip. Repair that accounting, the stale Intel macOS
    `HOST_LEGS` label, and `tools/tests/test_benchmark.py`'s target-catalog path
    (the owner is now the [backend target catalog](omega-rust/omega/backend/target/src/target_profile.rs)).

  Remaining runtime legs: Linux ARM64, Windows x64, Intel macOS and an applicable
  UEFI subject on QEMU/hardware. Exercise Windows job-object peak-memory
  accounting there. A subject with no bound entry is non-applicable to that
  pairing, not proof the host leg ran. Keep `structural_proofs` compile-only;
  `math_proofs` needs an authored entry/product choice before native measurement,
  without inventing useful runtime work for a proof-only program.

  Acceptance: records bind subject/compiler revision, actual exact enabled/
  disabled selection, admissions, target, measuring host and observed exit;
  report compile time, peak memory, code size and runtime with explicit skipped,
  unavailable or non-applicable reasons, and regenerate the matrix. Preserve
  `wrapping_square_sum`'s six-rule authored selection and separate compile-only
  evidence from matching-host execution. Schema validity alone establishes none
  of the missing execution or promotion evidence.

- **WORKSPACE-ROLLOUT.** Complete release evidence for any exact rule proposed
  for promotion under the [promotion contract](wiki/spec/build/optimizations.md#release-rollback-and-promotion).
  Keep the [inventory](omega-rust/omega/foundation/optimization-core/rules.md)
  explicit, opt-in and Experimental until the frozen-tree command
  `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace --no-fail-fast`
  passes and the owner separately approves that exact rule. Preserve doctests;
  `--all-targets` is not an equivalent command.

  The six candidates' incomplete evidence is now in the
  [promotion draft](wiki/drafts/measurements/optimizer_promotion_evidence.md).
  [promotions/](omega-rust/omega/foundation/optimization-core/promotions/README.md)
  contains no completed record, and the former architecture inventory/promotion
  gate no longer exists. Revalidate those historical pointers against current
  tests, artifacts and measurements; PER-RULE-COVERAGE owns missing behavioral
  evidence, BENCHMARKS owns measurement. Markdown completeness is not approval.

  Acceptance: the full command passes at a recorded clean revision; every
  promoted rule has reviewed semantic/corruption, differential/reference,
  determinism/bounded-work, target/OS, compile-time/output-quality and rollback
  evidence plus the explicit owner decision. Record completed reviews beside
  the inventory. No broad optimization level or implicit default is introduced.
