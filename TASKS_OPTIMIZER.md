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
  spans and FMA. macw11 measured why no further arm can be added today: only
  float/integer comparisons produce a Terminal intrinsic operation (float
  arithmetic, negation, conversion and builtins produce none; Terminal FMA is
  refused at `comparison_settlement.rs:315`), and production physical evidence
  is always fragment-scoped (`from_validated_fragment_publication` is the sole
  producer). `driver/tests/boundary_internal_call_evidence.rs` now witnesses
  `match_float_patterns`' two comparison occurrences binding complete physical
  children on macOS ARM64 and Linux x86-64; `boundary_operator_frontier.rs`
  additionally pins the stage-05 receiver-field float guard refusal
  (`prepare_expression.rs:268`). New physical coverage first needs those
  upstream producers: the stage-05 float field observation, stage-04 guard
  retention, and the Terminal float operations. Dynamic coverage has a
  different join:
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

  The Terminal codec (ae970cc6c1), native-artifact semantic-wrapper
  (ac20c488a9), image-emission object/scalar-call/hosted/placement
  (2e620b1271), image-crate executable/data-region and footprint-certificate
  (af926ae2) and provider-evidence component-era entry-ledger (cef536a369)
  families all run on the driver — the ledger first pinned typed rejection
  enums where its replays produced only diagnostic strings.
  `initialized_data_envelope` stays hand-written because its public API
  cannot recompute the inventory identity. Remaining: recheck surviving
  selected-dispatch source-edit matrices and any custody families formerly
  owned by compiler tests before scheduling a migration — deleted test
  targets do not establish equivalent coverage — then run the audit the
  acceptance names over inventories without matrix or driver consumers.

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
  [register allocation](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/README.md)'s
  `assignment/runtime_spill/` and selected-instruction
  `rewrites/runtime_spill/`; the latter independently replays stores, reloads,
  physical geometry and value lineage.

  Remaining work:

  - Admission: entry-bound victims with an edge back to the entry block now
    admit in `runtime_spill/admission.rs` — the boundary store re-executes on
    every arrival, a consistent plan cannot rewrite an entry register, and
    the store's own read keeps it live across the re-entry path — with
    `rewrites/runtime_spill/validation.rs` replay revalidating the route and
    `tests/parameters.rs` pinning the emitted program, its liveness, and the
    last-writer slot geometry under re-entry. Foreign-class values without a
    pure bit-transport pair still reject; the current scalar representation
    carries only Boolean, Integer and IeeeFloat, so vector/non-IEEE admission
    needs a representation and target capability first. IEEE transport and
    early-clobber tied uses with shared co-readers already work;
    `tests/read_modify_write.rs` pins a dedicated reload for the tied use.
  - Composition: fixed-view probe declines, pressure after materialized copies,
    and pressure after active-resident rematerialization already enter runtime
    spill with their validated prefix (`assignment/recovery.rs`). Residual
    pressure after completed literal-fold and pre-allocation runs also
    composes into runtime spill (e4dab11af5): `RuntimeSpillSource` carries
    SelectedLowering/PreAllocation arms that replay the run's custody as a
    prefix and bind its completion identity in the manifest. Remaining
    multi-recovery composition still goes through the same route and
    independent replay.
  - Slot reuse: emitted recovery uses eight-byte `LocalStorageSlotId::Spill`
    homes. `rewrites/runtime_spill/slot.rs` now reuses incumbent slots at
    storage-window granularity (6516219849): a reaching-definition fixpoint
    over pending stores unions observed definitions into windows, each window
    picks a fitting incumbent, and values without one take private slots;
    replay recomputes the assignment independently. The separate
    `assignment/stack_slot_coloring` and logical-spill boundaries have no
    sequenced runtime caller; do not mistake their validated plans for physical
    input or restore the removed logical-plan join. Retire superseded
    `unsequenced_spill_stages/` owners with their consumers instead of extending
    parallel slot-assignment implementations.
  - Stack provisioning: the hosted path already joined frame to demand, and
    `tests/artifacts/runtime_spill_stack_demand.rs` (84e088156d) pins it with
    real spill storage: a 40-live-value function takes runtime spill on three
    targets and its frame participates in stack demand, hosted-receiver
    supply, installation-record derivation and stale/insufficient rejection.
    External (non-hosted) entry roots are not yet covered. Two corpus
    programs exercise runtime spill natively on macOS ARM64:
    `collections/runtime_vec_u32_growth_exit` exits 70 and
    `calls/nested_call_tail_roots_exit` exits 0; `saturating_signed_divide_min_by_neg_one` no longer spills on
    arm64 (its `corpus_outcomes.txt` row is stale; the item now produces its
    x86-64 artifacts via the runtime split under ALLOCATION-REFINEMENT).

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
  [allocation recovery](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment/runtime_spill/recovery.rs)
  owns the pressure decision and subsequent assignment.

  `rewrites/runtime_split.rs` (d78956d764) adds the first runtime
  split: a `CopyI64` right after a pinned definition moves the value into a
  fresh unconstrained home, and stage-05 recovery tries it before
  rematerialization or spill (manifest tag `RuntimeSplit`, codec v15,
  independent replay refuses a spill recorded while a split was possible).
  `pass/arithmetic/saturating_signed_divide_min_by_neg_one` now produces
  linux_x86_64 and windows_x86_64 artifacts; its x86 execution and golden
  re-record wait on an x86 host. Known upstream bug the split worked around:
  stage 03 records the wrong defining instruction when it registers a result
  before emitting a helper (x86 division scratch, divisor copies) in
  `selection/{construction,validation}/scalar_graph/integer_arithmetic.rs`;
  the spill rewrite still trusts that origin and rejects such values.

  Existing cuts authenticate fixed uses; fixed-view copies already cover
  shared source exits. `split_domain_pressure` and its candidate tests score
  pressure across existing tied/edge-transfer domains, without inserting new
  split copies. Runtime recovery tries rematerialization and spilling; it
  does not implement arbitrary pressure-chosen splits. Recompute liveness,
  ranges and legality after changing the selected graph. Early-clobber
  participants are admitted to fixed-split requirements (a0a624873e), and
  the fixed-view copy source gate covers signed and unsigned 64-bit integers
  (3f3832dd2d), so the signed-divide to shift-count shape that drove the
  work now materializes; a scratch copy of it publishes a linux_x86_64 ELF
  and `saturating_signed_divide_min_by_neg_one` itself runs exit 70 on macOS
  ARM64. Extend the remaining tied-register, multiple-incoming-connector and
  cyclic restrictions while preserving exact register-unit aliases and
  target custody.

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
  [native frame realization](omega-rust/omega/compilation/backend/machine-emission/src/function_realization/frame.rs).
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
  Follow the ordinary [native admission route](omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md#ranked-native-admission):
  natural-ranked and unranked graphs retain their custody, and authenticated
  cyclic components remain frozen under optimization. No countdown-only fallback.

  Acceptance: execute the composed call on both Linux architectures; verify
  exact projection, referent and argument identity, callee ranking, return,
  cleanup and composed resources. Conflicting parent access and missing/invalid
  required ranking reject. Iterations add no frames; the outer call may have
  one. Changing-reference transfers additionally need represented loans/aliasing
  and ranking substitution; a fixed-referent witness or wider parameter count
  does not close them. Keep earlier source/lowering refusals with their owning
  task while carrying this customer through native replay.

- **JOIN-INVARIANT-REWRITE-TRANSPORT.** Let useful scalar rewrites survive
  when retained join invariants name the values being rewritten.
  Owners: stage 05's `proofs/scalar_block_invariants/joins.rs`, stage 06's
  `copy_propagation` and `dead_scalar_elimination`, and their independent
  Terminal rewrite validators.

  The refusal half is done. Both passes record a `PsiRewriteRefusal` on
  `PsiOptimizationStageResult::refusals()`, terminal production projects those
  onto `optimization_core::WithheldRewrite` in the shared
  `OptimizationReasonCode` vocabulary, `CompileReport::withheld_rewrites()`
  carries them out of both the Terminal and native routes, and the CLI names
  the exact rule and reason. `driver/tests/withheld_rewrite_reporting.rs`
  compiles a consumed-join customer and asserts both passes reach the report,
  with a selection-without-an-invariant control; the stage-level
  `consumed_literal_join_keeps_the_invariant_and_refuses_rewrites` and
  `unconsumed_literal_join_rewrites_without_refusal` remain the pass-local
  controls. The record stays producer-side: it is not encoded in the canonical
  execution record and the published module is identical either way.

  Remaining: the verifier-side proof-context transport that would let the
  rewrites survive instead of being withheld. A retained invariant must be
  able to move with the rewrite under independent checking, so the validator
  reconstructs the same proof question from the proposal rather than a
  different one. This is an optimization limitation, not a blocker to ordinary
  native execution.

  Acceptance: a consumed-join customer retains the selected rewrites through
  independently checked proof-context transport, and the withheld-rewrite
  report for it goes empty. Preserve exact proof questions, value/ranking
  identities and rejection of forged context changes; changing arm constants
  to avoid the invariant does not test the missing behavior.

  The driver fixture also records a separate native-emission gap worth its own
  customer: its shape reaches Terminal but fails image emission with
  `shared function contains an unaccounted operation`, with or without the
  dead scalar, so it is a driver fixture rather than a corpus one.

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
    argument transfers and claim transfers. Obligated structural-result calls
    now relocate with their discharged rosters (121bb2db8c), and a claim-free
    member `CallUnit` shared borrow of an operation result keeps the relocated
    case contained (0964c56169); all four borrow-call admissions tolerate the
    run's relocated-root discards. `CallStructuralScalar` shared borrows stop
    earlier, at the Terminal verifier's cyclic fence
    (`unranked_cycles.rs`), and `selected_evidence` carriers are unreachable
    from source inside loops (stage 04's `guarded_call_returns` builds only
    whole-machine plans). Whole-root Owned arguments produced by member
    establishments (`EstablishRecord`, `EstablishCase`,
    `EstablishScalarArray`, `CallStructural`) now copy into relocated calls
    when the root is unrestricted, claim-free, qualification-free and
    projection-free (0770fd8000); affine/linear, claimed, qualified and
    projected roots still reject. New cyclic source operations must pass
    ordinary Psi verification.
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
  [selected-stage optimization](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs).
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

  [Staged rewrites](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs)
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
  recovery. Owners: `rewrites/selected_lowering/literal_fold/`,
  `rewrites/constant_branch/`, `rewrites/copied_call_operand/` and
  [staged peepholes](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/peepholes/mod.rs).

  Literal nomination now scans every `MaterializeI64` per function
  (8cefe60b9f): producer and replay each re-derive sole-use custody and
  provenance, unprovable candidates decline, and
  `LiteralExtensionElimination`/`U12Load8IndexedOffset` are reachable from
  compiler-generated workloads. The `WrappingRemainder*` "unreachable" test
  comments (`literal_fold/pair_rule.rs`, `evidence_matrix/literal_folds.rs`)
  still explain reachability in classification terms and need re-checking.
  `COMPARE_ZERO`/`COMPARE_LEFT_ZERO` already select `CompareI64Zero`; the
  left-zero test covers equality readers and rejects ordering readers on both
  architectures. Preserve that behavior through the common descriptor route;
  do not restore a separate literal producer.

  `copied_call_operand` executes: its three retained-call rules moved into
  `rewrites/copied_call_operand/pair.rs` and drive
  `SelectedCopiedCallOperandFoldV1` through the pre-allocation joint fixed
  point, retaining the `CopyI64` producer, the ABI pins, implicit
  uses/definitions, clobbers, trap/control effects and the target's
  call-stack lifecycle while rebinding copied arguments — the
  transformation `copy_removal` deliberately refuses. Admission and replay
  independently require the rebound pins to coincide with every fixed view
  the source register already carries, because distinct views on one
  register are the entry transitions the completed step forbids. The
  evidence matrix's compiler-produced `Call` workload commits the
  transformation and replays it on linux_x64 and linux_arm64, and the source
  program `pass/optimizer/copied_call_operand_selection_exit` selects the
  rule by name in its build declaration, compiles, and runs to its authored
  exit result on a supported host.

  `terminator_pair` (a6268b2b4c) and `condition_materialization`
  (48c44018a6) are both done this way: their descriptors moved into
  `rewrites/constant_branch` and `rewrites/constant_boolean` and drive
  `SelectedConstantBranchFoldV1` and the executed constant-boolean fold,
  with admission and replay cross-checking the machine-effect catalog.
  `projected_access` is the last descriptor-only family: it carries pointer
  reads/writes and preserves architectural faults while absorbing an
  address displacement. The evidence matrix records that
  `SelectedAddressOffsetFoldV1` can never commit on compiler-emitted plans
  today (its producer needs consumers such as `Load8Indexed`,
  `ByteViewAddress`, `CopyBytes` and slot `Store64`); the descriptor stays
  staged until that production exists.

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
  `SelectedStoredLoadForwardingV1` now executes the existing byte-exact
  forwarding proposer and independent validator through the common preallocation
  catalog, with source/output/function/load identities carried into allocation
  manifest replay. The authored `optimizer/stored_load_forwarding_selection_exit`
  control exits 0 natively and interpreted; disabling the selection retains the
  field reload. Dead-store elimination and store motion remain under
  [rewrites/unexecuted/](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs),
  outside the executable catalog. Reuse their proposers and independent legality
  validators; EXACT-MACHINE-SIMPLIFICATIONS owns the common stage-execution
  join, not a second memory-specific pipeline.

  Establish the alias premise each admitted window needs. `commuting_accesses`
  currently treats different `StorageRoute::Place` identities as disjoint;
  the remaining memory rewrites likewise rely on the access roster's root/extent
  claims. Executable forwarding conservatively refuses intervening writes through
  distinct place identities, including forged edits presented directly to replay;
  same-place disjoint byte extents and private staging remain controls.
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
  `EstablishStructuralCase` already routes through stage 00
  (`routing.rs` → `structural_establishment.rs`); a source probe
  (`o in Outer::Wrap`) folds with no admission change. Zeroed establishments
  now prove scalar leaves and the first declared case (4c4bcf6610), and an
  `EstablishCase` payload copy proves member reads on the built case
  (2ecb9cce55) — both with optimization-unit-semantics replays.
  Stage-05/source frontiers confirmed by reprobe: runtime-index
  `StructuralCaseMembership` cannot reach stage 01 from any current source
  form, fixed-index integer reads on unestablished parameters decline by
  contract, and bounded indexed reads stop at stage 05's authored-binding
  check. Membership now crosses case-payload copies (2c086a6a): `PathStep`
  mixes ordinary structural steps with canonical copy-crossing steps and
  `Position` carries case payload-field rosters, in admission and replay.
  Still missing: ordinary-path copies (need field-name-to-id mapping first)
  and call-produced children (INTERPROCEDURAL-SUMMARIES).

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

  Rollback-to-identity coverage is restored.
  `driver/tests/rollback_to_identity.rs` supplies the real disable overlay
  through the compiler's own request and checks four things: the settled
  receipt reads `requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup];
  effective=[]`, the rolled-back build publishes the exact semantic module a
  build selecting nothing publishes, no overlay settles no receipt, and the
  rule demonstrably changes the module it is rolled back from. That last
  assertion is the one that keeps the comparison honest: on the corpus
  fixture's `machine main() { }`, and on a chain of single-successor states,
  `ControlFlowCleanup` leaves both the executable and the semantic module
  byte-identical, so equality under rollback would have restated the receipt.
  The fold needs a constant-condition transition whose untaken arm leaves a
  block unreachable, which is what the driver fixture pair authors.

  The corpus fixture `optimizer/rollback_to_no_selection_empty_entry` no
  longer claims otherwise: `corpus_runner::build_native_fixture` still builds
  its request with no overlay, and the fixture's comment now says so and
  points at the test. Existing CLI negatives cover invalid names and product
  restrictions. Comparing native executables cannot see a Psi-stage rule at
  all — emission normalizes `ControlFlowCleanup` away — so this axis compares
  `semantic_bytes()`.

  Composition coverage starts here. `driver/tests/composed_rewrites.rs` builds
  one fixture four ways — the authored `{CopyPropagation,
  DeadPureScalarElimination}` selection, each rule rolled back alone, and both
  rolled back — so only the compiler's own overlay differs and no source
  change can explain a module difference. It asserts that selecting both
  differs from each rule alone, which is the failure the axis exists to catch:
  equality with one of them means the other stopped reaching the module once
  its neighbour was selected. Both vacuity guards are live, and they earned
  their place — the first two fixtures silently had one rule not fire, a
  dead COPY rather than a dead pure computation. The modules measure 1079,
  1051, 985 and 957 bytes, so each saving is exactly its own, 28 and 94.

  A keyword screen of the six stage-06 rules' own tests, which is a screen and
  not proof, finds the same three axes absent for every rule: rollback,
  composition and budget. Fixed-point appears only for `control_flow_cleanup`.
  Publication is absent for `copy_propagation`, `global_value_numbering` and
  `sparse_conditional_constant_propagation`; corruption for
  `dead_scalar_elimination` and `global_value_numbering`; boundary for
  `control_flow_cleanup`. Rollback is covered for `ControlFlowCleanup` at the
  driver level only, which is why the stage files do not mention it. Budget is
  the one with no coverage anywhere and no customer yet.

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

  The evidence repairs landed at 2d0ef2edb5 with the compiler receipt fix at
  0f14a853dd: records are `omega-benchmark-record/2`, every compile adds a
  catalog probe disable so the compiler's own rollback receipt is reconciled
  against the authored selection, the record binds the binary's SHA-256,
  version and profile, an expected-exit mismatch refuses the measurement, the
  package review settles the requested target's document, and host legs stay
  pending until a measured runtime retires them (Intel macOS uses its
  canonical label). The first /2 rows are committed at d630b0a1e5 on macOS
  ARM64: `prime_counter` (exit 8), `cli_mvp` (exit 0),
  `wrapping_square_sum`'s authored six-rule selection (exit 0), and
  compile-only `standalone` and `structural_proofs`. Settlement now judges
  `omega.lock`'s presence rather than the resume command's exit, which
  reported "no pending package review" for dependency-free subjects after
  the first update already published the lock.

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
