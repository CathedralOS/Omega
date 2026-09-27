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
  for admitted operations/transfers on the common graph under
  [closed application and physical occurrence](wiki/spec/terminal-psi/boundary_calls.md#closed-application-and-physical-occurrence).
  Owners: `07_lowered-psi-to-terminal-psi/src/boundary_operator_custody.rs`
  and `native-artifact/src/physical/{operator_applications.rs,derivation/}`.

  For remaining intrinsic families, retain checked demand and exact Terminal
  occurrences before adding physical span arms. Current operator replay covers
  local initializers/FMA, structural returns and float/integer comparisons.
  Rebound scalar/unit calls now replay into `CheckedDynamicCallOccurrence`
  rows beside the D29 roster (descriptor-table lane); register-indirect forms
  remain. `derive_operator_physical_span` still returns `Ok(None)` — and the
  occurrence names `UnsupportedOperatorSpan` — for non-FMA/non-comparison
  `ExactCompilerIntrinsic` realizations and non-static-callee operations under
  checked-body realizations. The linw4 leg enumerated the fall-through and
  attributed every remaining family to an upstream wall rather than a
  witnessed physical-evidence gap: an installed-provider `BoundaryCall`
  reached native realization with `physical_evidence_gap() == None` and one
  evidence child, while float intrinsic families
  (`PrimitiveFloatBinary`, `NamedFloatNegation`, `NamedFloatConversion`,
  `BuiltinFunction`) cannot be produced from source — f64 literals/fields
  fail record-literal and pure-initializer admission, f64 operators record
  computation roots with no checked scalar expression, `Float::add` value
  calls do not resolve, and `sqrt()` hits a flow-call unit-plan omission —
  and register-indirect `CallDynamicParameter*` needs `&dyn` locals or
  arguments, both rejected in entry closures (`unsupported statement kind`,
  `call: call operation`). Parked probe: branch
  `swarm/linw4-translation-validation` commit `b9566d3148`
  (`tests/linw4_translation_probe.rs` compiles a self-authored boundary
  trait/provider through package-input helpers and inspects
  `physical_evidence_gap()`; branch `swarm/linw3-translation-validation`
  retains the earlier directed-float probe). Dynamic occurrence families and
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

  No claim roster reaches native lowering: no `ActivationClaim`/
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

- **GENERAL-LICM.** Extend invariant motion beyond the authenticated
  shared-source preheader in
  `01_abstract-operations-to-abstract-operations/src/ranked_rewrites/loop_invariant_scalar_motion/`.
  Keep independent admission/replay in `src/validation/`, including invariant
  operand substitution and the existing topology-based non-speculation gate.
  `132e9894d2` (macw9) added `StructuralByteSequenceFieldRead` relocation:
  its `index`/`length` operands are **proposition-pinned** — the accepted
  `index < length` obligation fact is re-derived from the moved op's current
  operand identities and pinned to the terminal recording, so substituting a
  member-parameter operand invalidates it even when the operand resolves to
  an invariant representative (member-op results in the same relocation run
  still qualify), and admission couples the read to a same-root
  `StructuralByteSequenceFieldLength`. Admission in
  `validation/place_observations.rs` + `invariant_operations.rs`, replay in
  `context/ranked_cycles/freeze/relocated_scalars.rs`.
  The boundary now schedules as `PsiOptimization::LoopInvariantScalarMotion`:
  selection resolves a session-level `PsiPassStep` that proposes, validates,
  and applies relocation candidates through `optimize_abstract_operations`,
  retaining declaration, decision, commit, and manifest rows under
  independent publication replay. The countdown-invariant-constant
  relocation boundary and the snapshot validators still compile only for
  tests and the crate's `test-support` feature.

  Remaining boundaries:
  - Establishment/custody families not covered by current relocation.
    Crash-continuation scalar calls already relocate
    through the authenticated shared-source preheader with the roster
    re-derived from the callee's verifier-owned contract via
    `substitute_crash_routes`. Still open:
    copyable unrestricted whole-root Owned arguments are already supported;
    affine/linear argument transfers need custody the boundary cannot currently
    re-express. Plain unrestricted claim-free structural results already admit;
    call motion carrying claim transfers, requirement obligations or selected
    evidence still lacks reconstruction of those
    relationships. Missing cyclic source operations must first pass ordinary
    Psi verification, not synthetic bypasses.
  - Profitability beyond the current execution-guarantee gate, keeping logical
    fuel distinct from optimization cost under
    [optimization semantics](wiki/spec/build/optimizations.md).
  - Motion needing new blocks or run duplication rather than the existing
    shared-source preheader, with corresponding block/occurrence evidence.

  Parked WIP: `swarm/macw6-general-licm` (one unvalidated commit 46a232ec0a,
  call-invariance test scaffolding) — review before resuming.

  Acceptance: each transformation independently reconstructs components,
  loop-carried custody, ranking, provenance, effects and fuel. Forged operands,
  retained internal discards, missing exit disposals and stale frontiers reject.
  Rebuild ownership membership after transformation and preserve Terminal-derived
  region custody; do not add a private loop forest or restore countdown-specific
  authority.

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
  [exact-rule inventory](omega-rust/omega/foundation/optimization-core/rules.md)
  lists the implemented `SelectedIncoming*` selections. `3e8da4d90e` (macw7)
  showed the descriptor's bound field carries value partitions:
  `PairImmediateBound::EncodingNonZero(4095)` on the immediate pairs versus
  `Exactly(0)` on new `COMPARE_ZERO`/`COMPARE_LEFT_ZERO` pairs restored the
  retired left-zero `CompareI64Zero` refinement, with the validator
  re-deriving the realization from the removed literal's recorded
  value/position. Every landed pair
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
  Owner: `omega-rust/omega/pipeline/04_selected-instructions-to-selected-instructions/`.
  Same-block copy removal, redundant-extension removal, address-offset folds
  (`9627b528bc`, `SelectedAddressOffsetFoldV1` through pre-allocation admission),
  and constant-boolean folds (`3f31f50e18`, `SelectedConstantBooleanFoldV1`,
  tag 48 / catalog row 4 / manifest tag 9 — `condition_state` lifted to
  `rewrites/` as shared flag-state vocabulary) already use the public catalog
  executor. Complete compare/test and scheduling families through that same
  route; preserve the existing `PreAllocationPolicy`.

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
  `04_selected-instructions-to-selected-instructions/src/rewrites/`;
  EXACT-MACHINE-SIMPLIFICATIONS owns their shared stage-execution join.

  Bind distinct-place non-aliasing to retained fact identities, or independently
  establish at the access-roster producer that the distinct places cannot
  overlap. Byte-view payload roster rows now bind the resolved backing storage
  root at the producer (`ByteViewHomes`), so subslice-view accesses charge the
  root instead of mislabeling the view identity. `f315e27e93` retains the
  unit's machine-scoped `ownership_frontier_facts` into
  `LegalizedScalarFunction` and encodes each fact identity into plan,
  scalar-graph, and validator identities; extend that premise binding to
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
  `01_abstract-operations-to-abstract-operations/src/{representation_specialization,field_value_specialization}/`.
  `92856482da` (macw8b) descended field-value proofs through stored-whole
  structural children (`Field` path segments, child carrier/type equality,
  owned empty-path access), and `ca85e84578` (macw8b) reseats producer-less
  places through uniform `bound_place` block-parameter bindings (all incoming
  edges binding the same whole empty-path argument, transitive with cycle
  guards, mirrored replay in `candidates/structural_bindings.rs`). A real
  completeness gap was also closed: `CallDynamicScalar`/`CallDynamicUnit`/
  `CallStoredDynamicScalar`/`StoreDynamicDescriptor` selection sources are
  now scanned for rewrites. `4c307fc9ed` (macw9) resolved the array-element
  frontier to coverage: `admission.rs`'s `descended_type` already resolves
  `FixedIndex`/`RuntimeIndex`/`Referent`/`Field` chains (`t[1]`, `r.inner[0]`,
  `self.arr[1]`, `m.arr[0].tag` all fold), and the commit pins the regression
  matrix plus decline/forged-evidence controls. Reachability bound discovered:
  an `EstablishRecord` child must be `Record`-shaped (or scalar/reference)
  transitively — no sum/array children — which bounds how far establishment
  proofs can descend through stored children, and `EstablishScalarArray`
  elements are scalar leaves (`establishment_scalar`'s `FixedIndex` bound is
  exact). Remaining bounds are upstream: `RuntimeIndex` memberships wait on a
  lowering scalar-control-plan (in-flight on `leaf/transition-arm-runtime-
  index-reads`), `IntegerStructuralField` under `FixedIndex` is unreachable
  from source today, and `EstablishStructuralCase` is still rejected at
  terminal-psi-to-abstract-operations.

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
  SPILL-REALIZATION owns executable recovery, final-frame provisioning and
  retiring superseded unsequenced spill boundaries with their consumers.

  The semantic-wrapper object's records, composition, validation and codec
  live in `native-artifact/src/semantic_wrapper_object`, and
  the plan's x86-64 template selection and replay live beside its recipe in
  `omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/encoding.rs`;
  native-realization keeps only the object stage's
  settlement/source/encoding custody join and replay. UEFI/provider/ABI gaps
  remain with their native owners, not another wrapper implementation.

  Audit surviving public entrances using qualified identities and repository-wide
  consumers; a common name such as `encode` does not identify a caller. Acceptance: the connected route has no
  competing entrances or orphan outputs, coordinators sequence typed stages,
  and retained plans constrain the physical operations they describe.
  An isolated validator or retained-but-unused plan does not close the join.

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
