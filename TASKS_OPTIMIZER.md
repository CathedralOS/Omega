# Optimizer tasks

Follow [board update discipline](TASKS.md#updating-the-boards): retain unfinished
work and its necessary context, not a history of completed milestones.
Use [working the board](TASKS.md#working-the-board) when this scope reopens.
Development iteration performance is a separate first-class outcome there;
it does not silently reopen optimizer feature work here.

**Parked.** This board is not open for assignment until the owner reopens it.
Do not claim or start these items; native correctness work that a program
needs stays on [TASKS.md](TASKS.md#immediate-product-closure).

This is the optimizer execution board, not its history. Architecture and
rationale live in [optimizer implementation](omega-rust/optimization.md);
landed milestones live in Git. Remove an item when its acceptance passes. An
open item states its current frontier once. Tag an added item's provenance on
its first line: `(new-scope)` or `(split-of:<parent-item>)`.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias. Every rule names the exact semantic, proof, ownership,
effect and target facts it consumes. Proof-carried facts may be consumed
in-process; no item here requires proof bundles, independent replay of the
compiler's own rewrites, or custody-mutation matrices (see
[PCC removal](TASKS.md#pcc-removal)). Optimizer crates have no
Rust tests; acceptance names corpus fixtures under `tests/omega/` run through
`tools/corpus_gate.py`.

## Execution priority

| Order | Work |
| --- | --- |
| 1. Correct native execution | GENERAL-CYCLIC-EXECUTION and SPILL-REALIZATION when a real program needs their call/storage route. |
| 2. Better code for runnable programs | GENERAL-LICM, ALLOCATION-REFINEMENT, machine/memory rewrites, representation specialization and interprocedural summaries. FRAME-LAYOUT follows TASKS.md **RUNTIME-SIZED-ACTIVATION-CONTRACT**; ordinary calls do not wait for it. |
| 3. Consolidation and rollout | PIPELINE-OWNER-CONSOLIDATION, PER-RULE-COVERAGE, BENCHMARKS, WORKSPACE-ROLLOUT. |

A concrete correctness regression takes precedence over this order. Mandatory
register-pressure recovery is not optional optimization quality; advanced
rewrites and promotion are not prerequisites for the first useful compiler.

## Register allocation and frames

- **SPILL-REALIZATION.** Finish executable register-pressure recovery and
  make its emitted frames participate in stack provisioning on every entry
  kind.
  - Owners: [register allocation](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/README.md)
    `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment/runtime_spill/`
    (pressure decision, `RuntimeSpillSource`) and stage 04
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/`
    (`omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/admission.rs`,
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/runtime_spill/slot.rs`: stores, reloads, window-granular slot reuse).
  - Reuse hosted-entry spill-frame accounting. Preserve
    `pass/collections/runtime_vec_u32_growth_exit` (exit 70) and
    `pass/calls/nested_call_tail_roots_exit` (exit 0) as regression controls.
  - Remaining: external (non-hosted) entry roots do not yet join spill frames
    to stack demand; values of a foreign register class without a pure
    bit-transport pair still reject (vector/non-IEEE needs a scalar
    representation and target capability first).
  Acceptance: a corpus pressure fixture with an external entry root executes
  natively with its spill frame counted in stack demand; slot reuse is counted
  once; insufficient stack supply rejects before execution (a byte ceiling is
  not backing). `python tools/corpus_gate.py --native --filter borrow/write_only`
  stays unchanged; no source crash route is added for spill operations.

- **ALLOCATION-REFINEMENT.** Add pressure-chosen live-range splits that move a
  value into an independently homed segment before falling back to storage.
  - Owners: stage 04
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/analyses/fixed_precolored_split_requirements/`,
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/` and
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/runtime_split.rs`; stage 05
    `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/assignment/runtime_spill/recovery.rs`
    (pressure decision; `split_domain_pressure` scores existing tied/edge
    domains but inserts no copies).
  - Current: `runtime_split.rs` splits a `CopyI64` right after a pinned
    definition; recovery tries it before rematerialization and spill. Empty
    whole-range domains also get an untied nonaddress integer fixed-use copy
    immediately before its consumer, preserving cyclic source parameters and
    rebuilding current graph facts. Control:
    `pass/arithmetic/runtime_cyclic_parameter_fixed_use_after_console_exit`.
    `pass/arithmetic/saturating_signed_divide_min_by_neg_one` runs exit 70 on
    all three native hosts through it.
  - Remaining: arbitrary pressure-chosen split points; tied-register,
    multiple-incoming-connector and cyclic restrictions. Recompute liveness,
    ranges and legality after changing the selected graph; keep exact
    register-unit aliases and fixed-use constraints.
  - Preserve stage 03 result origins at the first defining instruction after
    operand copies or scratch materialization. Regression control:
    `pass/arithmetic/runtime_pressure_trapping_add_exit` retains 32 arithmetic
    results and expects exit 70 through register-pressure recovery.
  Acceptance: a corpus pressure fixture runs natively on each admitted target
  and uses splits and copies where it previously spilled. SPILL-REALIZATION
  owns storage; this item adds no slot.

- **FRAME-LAYOUT.** Realize bounded activation claims in the native frame
  under the [activation storage contract](wiki/spec/resources/activation_storage.md).
  Depends on TASKS.md **RUNTIME-SIZED-ACTIVATION-CONTRACT** (source checking
  and Terminal claim production). Ordinary call frames do not wait for this;
  red-zone, probing, unwind, callee-save and call alignment have owners.
  - No claim roster reaches `PostAllocationMachineFunction`. Agree its
    transport with the upstream producer, then join claim sites to
    [native frame realization](omega-rust/omega/compilation/backend/machine-emission/src/function_realization/frame.rs).
    `compose_claim_bounds` in `omega-rust/psi/foundation/extents/src/activation_claims/mod.rs`
    computes aligned sums and branch maxima over supplied rows; it is not
    placement.
  - Implement placement and padding: disjoint offsets for co-live claims,
    reuse for mutually exclusive claims; bind demand to the final physical
    extent under [stack demand and backing](wiki/spec/resources/storage.md#stack-demand-and-backing).
  - Charge authored bounds, never runtime committed values. Keep each site's
    committed extent, stable addresses, reverse release order and suspension
    role in the final frame/probe/unwind plans. Eager frame commitment or lazy
    per-granule-probed commitment are both permitted; unbounded allocation is
    not.
  Acceptance: a run fixture accesses and releases claims natively on each
  admitted target; a claim whose committed extent exceeds its bound fails as a
  checked failure, not a trap or clamp.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Complete the native half of
  [ranked callees on projected receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers);
  TASKS.md's same-name item owns source production. Follow the
  [ranked native admission route](omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md#ranked-native-admission):
  natural-ranked and unranked graphs keep their custody, and cyclic
  components stay frozen under optimization. No countdown-only fallback.
  Changing-reference transfers additionally need represented loans/aliasing and
  ranking substitution. Keep
  `pass/control_flow/runtime_cyclic_reference_and_claim_rotation_exit` for
  changing projected references, call-returned linear claims, affine residuals
  and borrowed backing, with selected scalar-return cleanup after claim
  consumption. Its portable Terminal target roster does not replace
  the native host or resource qualification below.
  Acceptance: a corpus fixture executes the composed ranked call natively on
  linux_x86_64 and linux_arm64 with the correct projection, referent,
  arguments, return and cleanup; iterations add no frames (the outer call may
  have one); conflicting parent access and missing required ranking are
  rejected by checking (fail fixtures).

- **GENERAL-LICM.** Extend loop-invariant motion beyond the current operation
  and custody restrictions and the single shared preheader.
  - Owner: `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/ranked_rewrites/loop_invariant_scalar_motion/`,
    admission in `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation/invariant_calls.rs`,
    `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation/place_observations.rs` and
    `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation/invariant_operations.rs`. Scheduled as
    `PsiOptimization::LoopInvariantScalarMotion` through
    `optimize_abstract_operations`.
  - Remaining families: linear Owned argument transfers, claimed,
    qualified and projected roots, and claim transfers (unrestricted
    claim-free whole roots already copy; member-produced affine record,
    case, structural-call results and trivial affine locals consumed by
    member whole-root `Owned` arguments relocate with every consumer in
    one run, and affine records admit all-scalar field initializers).
    `CallStructuralScalar` shared borrows
    stop at the Terminal verifier's cyclic fence
    (`omega-rust/psi/compilation/representations/terminal-psi/verification/src/validation/control_flow/unranked_cycles.rs`).
    Linear `Owned` argument transfers additionally stand on cyclic claim
    custody that no stage admits today: `cycle_operation_eligible`
    requires `claim_transfers.is_empty()` on cyclic
    `CallUnit`/`CallStructuralScalar`, and `owned_call_result` requires
    both transfer rosters empty for an owned `CallStructural` result —
    the fence comment says claim movement stays closed until cyclic
    claim custody is supported. Upstream, a linear consumer's operand
    must chain from a call result (`05_checked-trees-to-lowered-psi`
    `result_uses.rs`) whose own linear result cannot be established in a
    state body (`04_typed-trees-to-checked-trees` `linear_validation.rs`),
    composed callee lowering refuses the local handoff's graph custody,
    and edge transport rejoins only machine-parameter claims
    (`state_graph/claims.rs`), which pinning then freezes. The narrowest
    enabling dependency is a block-confined claim: a member-local
    `EstablishRecord` consumed by a same-member `drop`/call, while
    `claim_transfers` stay fenced for any claim that outlives its block.
  - Structural predicates in crash-continuation rosters: structural-signature
    calls still require scalar-only predicates because canonical
    structural-path substitution is missing.
  - `StructuralByteSequenceFieldRead` needs same-root/path/field length facts
    and pinned index/length identities, not just invariant member parameters.
  - Profitability beyond the relocated-node-count estimate; keep semantic
    non-speculation, logical fuel and optimization cost distinct under
    [optimization semantics](wiki/spec/build/optimizations.md).
  - Motion that needs new blocks or run duplication, beyond the existing
    preheader.
  Keep topology-based non-speculation and region custody; do not add a
  private loop forest. Acceptance: corpus fixtures for each newly admitted
  family select the rule, hoist the operation, and run natively to their
  authored result; disabled selection runs identically.

## Machine optimization

- **EXACT-MACHINE-SIMPLIFICATIONS.** Make the retained selected-stage rewrites
  fire on source-produced code and add the staged ones to the executable
  catalog.
  - Owner: [selected-stage optimization](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs).
    `PRE_ALLOCATION_RULE_CATALOG` (`omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/catalog.rs`) already runs
    eight rules through `optimize_selected_instructions` (same-block copy,
    redundant extension, address offset, constant boolean/branch,
    dead/redundant/equivalent compare) under `PreAllocationPolicy`.
  - Address offset never fires on source code: real byte-field reads feed
    `AddressOffset` into `Load8Indexed`, outside the rule's admitted consumer
    table. Connect useful source-produced address windows. Dead-compare
    removal needs the constant-boolean/branch composition that removes flag
    readers.
  - [Staged rewrites](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs)
    have no catalog join for boundary boolean/branch folds or scheduling.
    Reuse `relocation` and `scheduled_relocation`; keep the distinct arm,
    inflow, confluence and memory-commuting premises and `interchange`'s
    two-run swap. Cross-block copy substitution and remaining compare/test
    vocabulary stay in scope. Respect x86-64 displacement bounds and AArch64
    scaling.
  - DECLARATIVE-PEEPHOLES owns descriptor generalization and literal
    nomination; ALIAS-AWARE-MEMORY owns memory rewrites.
  Acceptance: for each rule, a corpus fixture under `tests/omega/pass/optimizer/`
  selects it by exact name, shows it fired (an observable change in the
  emitted code), and runs to its authored exit natively; empty selection
  produces the unoptimized build. The existing
  `pass/optimizer/{constant_branch,dead_compare,equivalent_compare,redundant_compare}_selection_compile`,
  `copied_call_operand_selection_exit` and `stored_load_forwarding_selection_exit`
  fixtures run but do not yet show that their rule fired.

- **DECLARATIVE-PEEPHOLES.** Execute declarative relationships for physical
  register units, effects, traps, memory, stack and control flow through the
  selected-stage catalog.
  - Owners under `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/`:
    `selected_lowering/literal_fold/`, `constant_branch/`, `constant_boolean/`,
    `copied_call_operand/`, and [staged peepholes](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/peepholes/mod.rs).
  - `projected_access`
    (`omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/peepholes/projected_access.rs`) is the last
    descriptor-only family; it waits for source-produced address-offset
    consumers (`Load8Indexed`, `ByteViewAddress`, `CopyBytes`, slot `Store64`),
    see EXACT-MACHINE-SIMPLIFICATIONS.
  - Remaining: tied operands and further hosted-trap relationships. Keep
    independent axes for operand shape, unit roles, fault
    discharge/preservation and machine effects; do not multiply variants per
    combination. Recheck the `WrappingRemainder*` "unreachable" comments in
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/pair_rule.rs`.
  Acceptance: every required dimension has an exact selection and a rule that
  fires on x86-64 and AArch64 in a source-produced corpus fixture that runs to
  its authored result; unsupported relationships decline.

## Proof-, ownership-, and state-aware optimization

- **ALIAS-AWARE-MEMORY.** Run dead-store elimination and store motion through
  the selected-stage catalog, and widen forwarding with a real alias premise.
  - `SelectedStoredLoadForwardingV1` already runs
    (`pass/optimizer/stored_load_forwarding_selection_exit`, exit 0).
    Dead-store elimination and store motion remain under
    [rewrites/unexecuted/](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/mod.rs)
    (`omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/dead_store/`,
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/store_motion/`); join them through the common
    EXACT-MACHINE-SIMPLIFICATIONS route, not a memory-specific pipeline.
  - Alias premise:
    `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/unexecuted/commuting_accesses.rs` treats different
    `StorageRoute::Place` identities as disjoint, which is unsound in general.
    Forwarding refuses intervening writes through distinct places. Base
    distinct-place reasoning on loan/compatibility facts under
    [loan semantics](wiki/spec/terminal-psi/loans.md) (`ownership_frontier_facts`
    reach the legalized function but not selected access rows) or prove
    non-overlap at the access-roster producer. Exclusive custody alone does
    not prove projection disjointness; different view IDs are not disjoint
    bytes (`view_backing_roots` in
    `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection/byte_view_homes.rs`).
    Keep `omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/rewrites/place_storage.rs` as the shared route/extent vocabulary.
  - Control flow: forwarding across a join with no existing merge parameter,
    and store motion across non-reconvergent forks/joins, external
    predecessors or dynamic geometry, without adding or dropping writes.
  Acceptance: corpus fixtures select each memory rule, show it fired, and run
  natively; cover shared reborrows and field projections, intervening
  calls/effects, volatile/placed accesses and partial overlap, each running to
  the correct result.

- **REPRESENTATION-SPECIALIZATION.** Extend field/variant relevance in
  `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/representation_specialization/`
  and `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/field_value_specialization/`.
  - Covered today: proven membership, constant fields, nonconstant
    initializer forwarding, stored-whole child descent, uniform block
    bindings, zeroed establishments, `EstablishCase` payload copies, and
    membership/constant-field reads across ordinary-path
    `StructuralLeafCopy` results — source-level field identities resolve to
    the reached roster's `StructuralFieldId`, with `Case`, `RuntimeIndex`,
    `Referent` and `FixedByteRange` segments and non-structural carriers
    still declining.
  - Remaining: call-produced children
    (INTERPROCEDURAL-SUMMARIES). Type-path descent is not proof of stored
    contents. Runtime-index `StructuralCaseMembership` cannot reach stage 01
    from current source; bounded indexed reads stop at Psi stage 05's
    authored-binding check.
  - Corpus proof is blocked on routing: the pass executes in post-Terminal
    stage 01, but native realization receives only
    `project_post_terminal()` selections, which strip Psi-phase names, so
    no product route drives the pass and a rollback pair's `module:`
    comparison at the Terminal artifact cannot observe its rewrites.
    Until that projection owns the deferred Psi names, the fixture
    `pass/optimizer/ordinary_copy_membership_selected` pins the authored
    selection and machine shape without a `module:differs` guard.
  - Borrowed restoration windows (`MoveStructuralField`/`StoreStructuralField`)
    need the exact open/restore custody; a new window family needs its
    upstream operation contract first. Keep cyclic components frozen.
  Acceptance: a corpus fixture selects the specialization for each new case,
  shows the read retired, and runs to its authored result; disabled selection
  runs identically; unsupported paths decline.

- **INTERPROCEDURAL-SUMMARIES.** Add inlining and exact argument/result/
  place-access summaries in abstract-operations stage 01.
  - Reuse `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/analyses/semantic/effect_summaries.rs`
    (revision-bound transitive effects) and
    `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/analyses/control_flow/call_graph.rs` (call components, including known
    dynamic realizations and nominal cleanup calls). Neither gives precise
    callee reads/writes or an inlining transform; a coarse structural-state
    effect is not non-aliasing evidence.
  - Bind summaries to callee revisions, projected storage and aliases;
    invalidate them when the callee changes. Integrate inlining as an exact
    selection under [optimization semantics](wiki/spec/build/optimizations.md),
    preserving ownership, effects, cleanup and fuel.
  Acceptance: a corpus fixture selects inlining/summaries, the call is
  rewritten, and the program runs to its authored result; disabled selection
  runs identically.

## Pipeline cleanup follow-ups

- **PIPELINE-OWNER-CONSOLIDATION.** Finish the public-entrance and consumer
  audit under the [connected pipeline contract](omega-rust/pipeline.md):
  remove competing entrances and orphan outputs, and make retained plans
  drive the physical operations they describe. A shared name such as
  `encode`, an isolated validator, or a retained-but-unused plan is not a
  join.
  - The common physical route is
    `omega-rust/omega/compilation/driver/src/native/physical_pipeline.rs` plus
    machine-emission's frame realization
    (`omega-rust/omega/compilation/backend/machine-emission/src/function_realization/frame.rs`);
    native-artifact owns `publish_fragment_object`
    (`omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs`).
  - Catalog execution stays with EXACT-MACHINE-SIMPLIFICATIONS,
    ALIAS-AWARE-MEMORY and DECLARATIVE-PEEPHOLES; spill-boundary retirement
    with SPILL-REALIZATION.
  Acceptance: no competing entrances or orphan products; empty and nonempty
  optimization selections share one physical route; the native corpus is
  unchanged.

## Verification and rollout

- **PER-RULE-COVERAGE.** Give each executable exact rule corpus coverage
  through its public stage entrance: exact selection, a positive case that
  fires, negative and boundary cases that decline, empty/disabled identity,
  rollback, composition with its neighbours, budget, and determinism. Follow
  the [stage-extension matrix](omega-rust/optimization.md#validation-when-extending-a-stage)
  minus its replay/corruption axes. Keep the
  [rule inventory](omega-rust/omega/foundation/optimization-core/rules.md)
  tied to current catalogs and fixtures.
  - Reuse the corpus runner's `rollback.txt` support and rolled/unrolled
    Terminal comparison. `pass/optimizer/rollback_to_no_selection_empty_entry`
    covers a request superset with an inert selected rule (`module:same`);
    `rollback_control_flow_cleanup_selected` is the non-vacuous
    `module:differs` control. Audit the remaining rule inventory against these
    axes, including `{CopyPropagation, DeadPureScalarElimination}` composition;
    do not rebuild overlay support or count a no-op as evidence of a rewrite.
  - A Psi-stage rule is invisible in the native executable (emission
    normalizes `ControlFlowCleanup` away); its fixture needs a constant
    condition whose untaken arm leaves a block unreachable, and must compare
    the semantic module, not the executable.
  - Add a budget control only for a named bounded operation; do not invent
    an optimizer-wide budget mechanism to fill a coverage cell.
  Acceptance: each applicable axis runs as a corpus fixture using the existing
  rollback overlay; each fixture shows the rule fired or was
  rolled back, since a green run alone proves neither. Non-applicable axes are
  explained (mandatory recovery has no disabled mode).

- **BENCHMARKS.** Complete subject/selection/target measurements through the
  existing [prepare/measure/validate/matrix workflow](tools/benchmark/README.md)
  and [benchmark matrix](wiki/drafts/measurements/benchmarks.md). Do not
  rebuild the harness. Records (`omega-benchmark-record/2`) exist for macOS
  ARM64 and Linux x86-64; they describe their recorded revisions, not a
  current baseline.
  - Remaining runtime legs: Linux ARM64, Windows x64 (exercise job-object
    peak-memory accounting), Intel macOS, and an applicable UEFI subject on
    QEMU or hardware. A subject with no bound entry is non-applicable, not a
    run leg.
  - Keep `structural_proofs` compile-only; `math_proofs` needs an authored
    entry/product choice before native measurement.
  Acceptance: records bind subject and compiler revision, exact
  enabled/disabled selection, target, measuring host and observed exit; report
  compile time, peak memory, code size and runtime with explicit skipped,
  unavailable or non-applicable reasons; the matrix is regenerated.
  `wrapping_square_sum` keeps its six-rule authored selection; compile-only
  evidence stays separate from matching-host execution.

- **WORKSPACE-ROLLOUT.** Complete release evidence for any exact rule proposed
  for promotion under the
  [promotion contract](wiki/spec/build/optimizations.md#release-rollback-and-promotion).
  Keep the [inventory](omega-rust/omega/foundation/optimization-core/rules.md)
  explicit, opt-in and Experimental until the frozen-tree command
  `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace --no-fail-fast`
  passes (doctests included; `--all-targets` is not equivalent) and the owner
  approves that exact rule. The six candidates' incomplete evidence is in the
  [promotion draft](wiki/drafts/measurements/optimizer_promotion_evidence.md);
  [promotions/](omega-rust/omega/foundation/optimization-core/promotions/README.md)
  holds no completed record. PER-RULE-COVERAGE owns behavioral evidence,
  BENCHMARKS owns measurement.
  Acceptance: the full command passes at a recorded clean revision; every
  promoted rule has reviewed semantic, differential/reference,
  determinism/bounded-work, target/OS, compile-time/output-quality and rollback
  evidence plus the owner decision, recorded beside the inventory. No broad
  optimization level or implicit default is introduced.
