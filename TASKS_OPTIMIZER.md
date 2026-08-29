# Optimizer Tasks

This is the execution checklist. Architecture and rationale live in
[`wiki/design_briefs/optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md)
and its linked briefs. Language-semantic blockers alone belong in
[`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias while executing this plan.

## Status legend

- `[x]` implemented and tested at its current boundary;
- `[>]` active slice;
- `[ ]` not yet implemented; and
- `[?]` requires an owner language decision.

## Current stopping point

[>] Add `X86SelectXorZeroI64MaterializationV1`: replace x86-64
`MOV r64, imm64(0)` with canonical `XOR r64, r64` only when independent
selected-CFG liveness reconstruction proves every canonical RFLAGS unit dead.

This slice also repairs the physical-pipeline organization exposed by the new
rule. It must add a catalog-driven post-allocation entrance and one typed stage
result, not another bespoke route/carrier chain copied from AArch64 MOVN.

Checkpoint complete: exact build vocabulary, the target-owned canonical x86
encoder/decoder, the independently replayed symbolic rule, and the
catalog-driven post-allocation stage entrance/typed result. The next checkpoint
starts at layout-independent encoding; x86 optimized bytes are not yet admitted
to the complete physical route.

Acceptance:

- explicit build opt-in and empty-selection baseline;
- target-owned canonical three-byte encoding/decoding/effect reconstruction;
- symbolic producer plus independent liveness validator;
- low/high GPR, REX, ModRM, flags-live, corruption, and budget coverage;
- exact ten-to-three-byte layout accounting;
- custody through whole-function realization and artifacts;
- direct and selected-lowering-composed paths;
- small meaningful stage entrances and no new monolith; and
- workspace checks, checkpoint, rebase, and push.

## Completed foundation

- [x] Exact source-visible `Optimization` vocabulary and versioned canonical
  selection encoding/identity.
- [x] Empty selection bypasses optimizer construction; human report selection
  is separate.
- [x] Toolchain-provided `build.omg` exact enable surface with duplicate
  rejection.
- [x] Complete validated optimization unit and retained facts/provenance/fuel.
- [x] Rule, validator, candidate, pass, fact, ledger, report, and artifact
  identities/codecs.
- [x] Bounded deterministic pass manager with ordered catalog and analysis
  invalidation auditing.
- [x] Independent Psi candidate and complete-unit validation.
- [x] Psi CFG products: graph, dominators, postdominators, SCCs, loops, and call
  graph.
- [x] Psi exact pass families: control-flow cleanup, SCCP, copy propagation,
  GVN, dead pure scalar elimination, and proof-check elision.
- [x] Proof-certified exact scalar identities retain accepted-obligation
  custody through lowering.
- [x] Selected-lowering exact incoming u12 add/subtract folds.
- [x] Physical register model, liveness, live ranges, allocation legality,
  transition-free homes, fixed-view recovery, and bounded rematerialization
  slices.
- [x] Symbolic post-allocation plan/effects and independent validation.
- [x] AArch64 compare-zero/branch-nonzero CBNZ fusion.
- [x] AArch64 shortest MOVN-seeded i64 materialization.
- [x] x86 conditional-branch rel32-to-rel8 function-relative relaxation.
- [x] Encoding, layout, whole-function exit, realization, fragment, text,
  object, optimized artifact, and callable-entry custody slices.
- [x] Optimizer crates reorganized into analyses/planning/rules/stages with
  focused leaf modules and mirrored tests.

## Organization gate

- [x] Replace `post_allocation_machine_optimizations.rs` with a folder whose
  entrance owns the exact stage catalog and typed result; descend to
  `aarch64/{cbnz,movn}` and `x86_64/xor_zero` custody leaves.
- [>] Split `post_allocation_selected_form_encoding.rs` by stage model,
  row/structural encoding, machine-rule disposition, identity, and independent
  validation. Its entrance must own the encode/validate join.
- [ ] Remove optimization-name-specific variants from complete physical route
  results once the typed post-allocation result can carry them.
- [ ] Derive or exhaustively cross-check build vocabulary and report mappings
  against `Optimization::ALL`; no hidden second registry.
- [ ] Keep preferred entrance files below 100 lines. Any entrance above 200
  lines needs a documented semantic reason. No optimizer production file may
  exceed 1,500 lines or mix catalog, unrelated rule mechanics, validator, codec,
  and broad fixtures.
- [x] Split the architecture brief into a real entrance plus semantic, rule
  engine, physical pipeline, source organization, and rollout briefs.
- [x] Compact this file to an executable checklist; detailed design is not a
  task-list responsibility.

## P0 — Opt-in and compatibility firewall

- [x] Exact named build selections and canonical order.
- [x] Duplicate, unknown, noncanonical, trailing, and old-version rejection.
- [x] Full selection identity retained across phase projections.
- [x] Empty selection preserves ordinary compilation.
- [ ] Add golden canaries comparing no-selection source acceptance,
  diagnostics, interpreter output, native bytes, and artifact metadata on every
  supported host/target pair.
- [ ] Add an exact per-rule disable/rollback path to release tooling.

## P1 — Shared rule engine and analysis system

- [x] Stable rule contracts, safety classes, required analyses, invalidations,
  budgets, decisions, typed facts, and manifests.
- [x] Deterministic Psi catalog, scheduling, fixed point, and replay.
- [x] Revision-aware analysis cache with stale-analysis tests.
- [>] Add a similarly obvious ordered catalog at every physical stage; machine
  optimization currently has well-shaped rule leaves but pipeline dispatch is
  still duplicated.
- [ ] Unify common catalog descriptors without erasing representation-specific
  candidate and validator types.
- [ ] Add catalog coverage tests proving every selected name is scheduled once
  or rejected for an explicit phase/target reason.

## P2 — Validation and publication

- [x] Representation and rule-level independent validators.
- [x] Identity-bound decisions, pass records, manifests, and work usage.
- [x] Source-to-optimized Psi projection and lower-stage custody checks.
- [ ] Complete translation validation for all lowering and machine rule
  families.
- [ ] Add generated differential testing across interpreter/reference native
  execution for exact integer, float, trap, atomic, placed-memory, cleanup, and
  transition cases.
- [ ] Add end-to-end mutation tests for every manifest/custody field.

## P3 — Psi optimizer

- [x] Control-flow cleanup with independent graph reconstruction.
- [x] SCCP with exact range/constant facts.
- [x] Copy propagation with dominance, ownership, and effect barriers.
- [x] GVN for local, dominating, and phi-translated expressions.
- [x] Dead pure scalar elimination using a closed operation partition.
- [x] Proof-check elision and proof-certified exact integer identities.
- [ ] Extend GVN and scalar identities to additional exact operations only with
  exhaustive producer/validator partitions.
- [ ] Implement loop-invariant code motion after cyclic Terminal-Psi semantics
  are resolved.
- [?] Define suspension/resume edges for CFG analyses.
- [?] Define whether cyclic control flow is admitted in Terminal Psi.

## P4 — Lowering optimizer

- [x] Target/legalized operation and selected-instruction validation.
- [x] Exact incoming u12 add/subtract immediate folds.
- [ ] Generalize legalization into an ordered declarative catalog of target
  forms, constraints, costs, and validators.
- [ ] Add exact address-mode folding, compare/branch selection, extension
  elimination, and constant materialization rules one named family at a time.
- [ ] Validate ABI operands, calls, clobbers, effects, traps, provenance, and
  logical fuel across every selected rule.

## P5 — Register allocation and frame assignment

- [x] Selected-CFG liveness and live-range fragments.
- [x] Register views/units, aliasing, availability, and allocation legality.
- [x] Transition-free home assignment and post-allocation manifest.
- [x] Exact fixed-view copy and active-resident rematerialization recovery
  slices.
- [ ] Replace the remaining narrow allocator with a general deterministic
  interference allocator.
- [ ] Add spill choice, insertion, reload/store validation, and stack-slot
  coloring.
- [ ] Add coalescing, live-range splitting, fixed/precolored intervals, and
  rematerialization cost decisions.
- [ ] Implement frame layout, alignment, red-zone/shadow-space, unwind, probing,
  stable-address loans, and dynamic-allocation constraints.
- [ ] Add x86-64 and AArch64 ABI/call-clobber corruption matrices.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic machine plan and effects.
- [x] AArch64 CBNZ fusion and MOVN materialization rules.
- [x] x86 rel8 layout relaxation.
- [>] x86 XOR-zero materialization with RFLAGS-dead proof.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling rules where independently verifiable.
- [ ] Add target cost models as non-authoritative identities; semantic
  validation must not depend on cost estimates.
- [ ] Generalize whole-function encoding/layout/realization so new form
  substitutions add one rule leaf and catalog entry, not a new route family.

## P7 — Proof-, ownership-, and state-aware optimizations

- [x] Accepted-obligation identities can authorize exact proof-check and scalar
  rewrites.
- [ ] Alias/borrow-aware load forwarding, dead-store elimination, and mutation
  motion.
- [ ] Field/variant relevance and invariant-window specialization.
- [ ] Cleanup and transition reachability pruning.
- [ ] State-argument/result specialization with edge provenance.
- [ ] Interprocedural service/call summaries and proof-bound inlining.
- [ ] Proof-directed loop bounds, induction simplification, and vectorization
  with exact lane semantics.

Each rule must name the exact proof/ownership facts consumed and retain their
identities in the decision and publication chain.

## P8 — Search and ML extensibility

- [x] Identity vocabulary for workload profile, decisions, cost model, rule
  set, selections, and ledger.
- [ ] Versioned model input schema containing source/target/rule/fact features
  without raw pointers or unstable insertion order.
- [ ] Versioned output schema naming existing candidate identities plus scores
  or decisions.
- [ ] Record-only mode that cannot change baseline output.
- [ ] Deterministic replay with exact identity mismatch rejection.
- [ ] Sandboxed external policy boundary with timeout/resource limits and an
  explicit fallback.
- [ ] Offline corpus capture, training, evaluation, and regression tooling.

ML may rank declared equal transformations. It cannot invent an unvalidated
rewrite or opt a program into lossy floating-point semantics.

## P9 — Testing, stabilization, and rollout

- [ ] Per-rule positive, negative, boundary, disabled, budget, determinism,
  idempotence, and corruption suites.
- [ ] Cross-rule phase-composition matrix, including deliberate fail-closed
  unsupported combinations.
- [ ] Randomized valid-Psi and selected-machine differential corpus.
- [ ] Supported target/OS allocator, encoding, unwind, object, and callable
  matrix.
- [ ] Compile-time, memory, code-size, and runtime benchmarks with versioned
  non-authoritative evidence.
- [ ] Exact-rule release notes and rollback procedures.
- [ ] Owner-reviewed promotion criteria per rule; never implicit broad levels.

## Near-term execution order

1. [x] Finish the x86 XOR-zero leaf encoder and symbolic rule.
2. [x] Introduce the post-allocation stage catalog and typed result.
3. [>] Split the encoding stage monolith along that taxonomy (the machine stage
   split is complete).
4. Carry the generic result through encoding, layout, realization, and artifact
   custody with exact byte-delta tests.
5. Add build opt-in and full direct/composed integration tests.
6. Run focused crates, pipeline tests with the required stack, workspace check,
   and diff checks.
7. Update this stopping point, commit without the unrelated checked-tree edit,
   rebase on upstream, rerun affected checks, and push.
