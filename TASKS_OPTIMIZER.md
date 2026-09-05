# Optimizer tasks

This is the optimizer execution board, not its history. Architecture and
rationale live in
[`wiki/design_briefs/optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md),
and landed milestones live in Git. Remove work from this file when its
acceptance condition passes.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias. Every rule must name the exact semantic, proof, ownership,
effect, target, and provenance facts it consumes, and retain the identities
needed for independent replay through publication.

## Immediate gate

- **PIPELINE-PHASE-INTEGRATION.** Execute the four
  [ownership cleanup packages](wiki/architecture/pipeline/ownership_cleanup.md):
  (1) delete umbrella/catch-all owners and consolidate pipeline crates;
  (2) finish all Omega/Psi representation roots and current-data ownership;
  (3) delete the alternate physical route; (4) execute selected Psi optimization
  before Terminal. Work in that order, moving data with owners where needed.
  Whole-owner removal and real behavior are milestones; helper extractions are
  prerequisites. Keep discoveries inside these moves and delete completed work.
  Acceptance: all four finish conditions hold, including preserved ordinary,
  ranked and callback behavior, empty/nonempty selection on one graph, standalone
  Psi and separately authorized resumed lowering, and independent replay.
  Folder renames, common wrappers and identity-only passes do not close this item.

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
  every admitted lowering and machine-rule family. Existing scalar, Unit,
  constant, call, and selected machine families are landed; remaining source
  families and publication routes must fail closed until their exact grammar,
  target applicability, result, effect, cleanup, and custody facts reconstruct.

  Close D32 with one source-reachable selected-lowering operation carrying
  nonempty physical evidence through allocation, layout, native emission, and
  independent replay. Bind the immutable Terminal product, validated
  optimization projection, and each surviving boundary occurrence to exactly
  one physical child with its D29 application or D41 settlement parent.
  Acceptance: post-Psi optimization
  and final publication reject missing, duplicate, stale, substituted, padded,
  or role-swapped children; only verified eliminated occurrences need no child.
  General calls depend on `FRAME-LAYOUT` and `GENERAL-CALL-CLOBBERS` below.
  These are native compiler guarantees, independent of package locks or
  `PackageInstance` construction.

- **BOUNDARY-QUALIFICATION-PRESERVATION.** Add optimizer and publication
  controls for boundary requirements that consume carried structural-domain
  qualifications. Joins take at most the intersection; CSE/GVN must distinguish
  unequal rosters; no transformation may widen a roster or mint a routed
  qualification. Acceptance: dropped, substituted, redirected, or widened
  qualification identities reject through final publication.

- **GENERATED-DIFFERENTIALS.** Extend same-artifact interpreter/native
  differential testing beyond the landed exact-integer lane to float, trap,
  atomic, placed-memory, cleanup, and transition behavior. Acceptance: fixed
  seeded corpora replay deterministically and native observations equal the
  reference interpreter under the same observation profile.

- **CUSTODY-MUTATION-COVERAGE.** Complete authenticated one-field mutation
  tests for every remaining manifest, receipt, codec, and artifact-custody
  family. Acceptance: each representable field can be changed independently,
  the containing identity can be recomputed, and independent replay still
  rejects the substitution.

## Psi optimization and loops

- **GENERAL-CYCLIC-EXECUTION.** Generalize the exact unsigned-countdown carrier
  to ordinary cyclic Terminal Psi with authenticated SCCs, dominance and
  frontiers, optional well-founded ranking, productive unranked components,
  and structured finite-work failures.

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
  publication replay.

- **SELECTED-ABI-VALIDATION.** Validate ABI operands, calls, clobbers, effects,
  traps, provenance, cleanup, and logical fuel across every selected rule.

## Register allocation and frames

- **SPILL-REALIZATION.** Complete deterministic spill choice, physical slot
  assignment/coloring, store/reload insertion, later-use rewrites, and
  independent validation. Existing logical spill plans grant no frame, unwind,
  instruction, or publication authority.

- **ALLOCATION-REFINEMENT.** Complete coalescing, live-range splitting,
  fixed/precolored intervals, and rematerialization cost decisions while
  preserving exact register-unit aliases, liveness, and target custody.

- **FRAME-LAYOUT.** Extend exact nonzero-frame realization beyond the current
  single-block and canonical three-block/two-return System V AMD64/AAPCS64
  family: broader CFG admission, Microsoft shadow space, red-zone policy,
  probing, unwind information, stable-address loans, and dynamic-allocation
  constraints. General calls need target-owned frame, callee-save,
  link-register, and call-site alignment plans. Acceptance: every admitted frame
  policy replays its exact physical accesses through callable publication;
  requirements artifacts
  remain non-authoritative until that replay succeeds.

  An authored empty machine selected for optimization resolves to
  `CanonicalFixedFrameBodyV1`, not the Unit baseline route, so
  `validate_unit_shape` in
  `backend/machine-emission/src/function_realization/unit/source.rs`
  rejects the selected plan an ordinary `machine Main::launch() {}` produces.
  The fixed-frame route takes the AAPCS64 leaf exemption
  (`ReturnAddressFrameCustody::LiveLinkRegister`, zero frame), so its AArch64
  function is a bare `ret` and `image-emission` rejects it as
  `MissingAarch64UnitReturnLink`; every function that route can currently
  publish carries `unit_stack` evidence, which the object boundary requires a
  saved link register for. Establish which of the two is wrong - the Unit
  shape test, or the leaf exemption for a Unit-bodied function - before
  changing either; broadening the fixed-frame policy would also reframe
  AArch64 scalar leaves, which the boundary admits frameless today.
  Acceptance: `optimized_empty_machine_reaches_object_custody` in
  `terminal-psi-to-native-artifact` covers `linux_arm64` as well.

- **GENERAL-CALL-CLOBBERS.** Extend live-across-call allocation and clobber
  validation from the landed attached-Unit fork/join slice through general
  scalar and structural calls on each ABI.

## Machine optimization

- **DECLARATIVE-PEEPHOLES.** Generalize the landed bounded instruction-pair
  descriptors to symbolic instructions, physical register units, effects,
  traps, memory, stack, and control flow without replacing the independent
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

  `omega --target <name>` publishes no retained native artifact on any target:
  `image/src/output.rs` leaves `compiler_function_validation` `None` and
  the workspace has no producer for it, so `publish_retained_native_artifact`
  stops at `native publication requires compiler-function validation evidence`.
  Acceptance: an empty-entry sample publishes an executable on each supported
  target with that evidence derived, not omitted.

- **BENCHMARKS.** Publish versioned compile-time, peak-memory, code-size, and
  runtime benchmarks keyed by exact rule selection and target.
