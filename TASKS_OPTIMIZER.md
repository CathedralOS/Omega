# Optimizer Tasks

This is the executable work list. Architecture and rationale live in
[`wiki/design_briefs/optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md).
Git history is the completed-milestone archive; completed work is summarized
here instead of repeated chronologically.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias.

Status: `[x]` complete, `[>]` active, `[ ]` pending, `[?]` owner language
decision. Only true language-semantic questions belong in
[`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).

## Now

1. [>] Restore the source-navigation contract. Complete the organization work
   below before adding another broad optimization family.
2. [ ] Split the fixed-view-copy artifact architecture from v4 to v5. V5 must
   carry `SelectedInstructionPlan::structural_unit_functions`; v4 must remain
   byte-stable and decode that field as empty.
3. [ ] Add abstract-to-target translation validation for
   `WrappingIntegerAdd(parameter, parameter)`, then wrapping subtract and
   multiply as separate exact catalog rows and leaves.
4. [ ] Repair the four stale optimization-pipeline proof fixtures whose exact
   add/subtract goals are still represented by `KernelDerived(Truth)`. Use
   checked certificate-derived proofs; do not weaken proof admission.
5. [ ] Finish workspace validation and rollout canaries before promoting any
   rule beyond explicit opt-in.

## P0 — Source navigation and taxonomy

- [x] Define the strict entrance/catalog/exact-leaf contract and map it to the
  useful Squalr registry -> plan -> dispatcher pattern.
- [x] Replace the historical 651-line source-organization narrative with a
  concise current contract, honest debt audit, stage inventory, and refactor
  triggers.
- [x] Compact this file from a milestone ledger into an executable checklist.
- [x] Split the 1,254-line source-organization guard into a tiny audit entrance
  over `inventory`, `bounds`, `entrances`, `catalogs`, and `retired_paths`.
  One six-row stage descriptor now carries entrance, catalog, coordination
  marker, and next rungs together while preserving the single aggregated test.
- [x] Make every executable Psi pass `mod.rs` a meaningful pass entrance. The
  stage-wide `rules/catalog.rs` remains the sole exact-selection enable/disable
  table; each pass entrance now owns its visible local rule order instead of
  re-exporting a sibling catalog function.
- [x] Split the 967-line optimization manifest into a 37-line wire-family
  entrance over decision-v5, pass-v1, work usage, fact reference, framing, and
  error leaves. Its tests now mirror those record families, and the guard
  prohibits restoring the flat leaf.
- [x] Remove the legalized-call validation forwarding wall. Its small
  `validation/mod.rs` now owns the complete representation-validation join.
- [x] Replace the 943-line abstract-operation identity encoder with a 71-line
  exhaustive family router over structural establishment, calls/effects,
  scalar operations, control exits, and scalar-operation shapes. Shared
  canonical carriers now sit beside the identity entrance; exact tags and
  field order remain in focused family leaves.
- [x] Replace SCCP validation's forwarding wall and 845-line candidate leaf
  with a 98-line rule-first, exhaustive-patch entrance over integer, boolean,
  observation, and range-comparison validation leaves.
- [ ] Audit all other `lib.rs`/`mod.rs` files and explicitly classify each as
  crate map, stage group, or executable entrance. Only executable entrances
  need a real join; no executable entrance may be a forwarding wall.
- [ ] Ratchet production leaves toward 600 lines. The current governed audit
  has 12 production-classified leaves at 750+ lines. Continue with independent
  live-range replay (881), then GVN validation, machine effects, MOVN, spill
  choice, selected validation, ledger, and lowering control/settlement leaves.
- [ ] Ratchet focused tests and fixtures toward 800 lines. The current governed
  audit has 14 test/fixture leaves at 1,000+ lines; first split structural
  catalog, register-allocation, and selected-lowering matrices by exact
  artifact/rule family and behavior.
- [x] Replace parallel rule-stage path arrays in the organization guard with
  typed stage descriptors and generic entrance/catalog/next-rung checks.
  Bespoke checks remain only for genuinely stage-specific invariants.
- [ ] Keep the hard migration ceilings at 1,000 production and 1,500 tests
  until the ratchet is complete. Crossing the 600/800 refactor target creates
  explicit non-growing debt; it is not considered healthy organization.

## P1 — Opt-in, catalogs, and compatibility firewall

- [x] Exact named build selections with canonical order and identity.
- [x] Empty selection preserves the ordinary compiler path and constructs no
  optimizer machinery.
- [x] Duplicate, unknown, noncanonical, trailing, and incompatible-version
  selections fail closed.
- [x] Exact per-rule release rollback is subtractive and never enables a rule.
- [x] One owning catalog per Psi, selected-lowering, allocation-recovery,
  post-allocation-machine, and function-relative-layout phase.
- [x] Catalog coverage proves each public exact name is scheduled once or has
  an explicit phase/target rejection.
- [x] No-selection canaries cover source acceptance, diagnostics, interpreter
  output, native bytes, and artifact metadata.

## P2 — Validation, translation, and publication

- [x] Producers create immutable plans; independent validators reconstruct
  representation and rule preconditions before publication.
- [x] Decisions, work usage, manifests, facts, and custody receipts are
  identity-bound.
- [x] Psi candidate declarations retain applied and skipped decisions with
  independently replayed manifest, rule, revision, and policy evidence.
- [>] Complete independent translation validation for every lowering and
  machine-rule family. Sixteen abstract-to-target families, selected incoming
  u12 folds, current machine substitutions, structural-Unit encoding, resolved
  layout, and ranked-u32 publication routes are covered; the remaining source
  families and publication routes are not.
- [ ] Add fixed-view-copy v5 as a versioned envelope and structural selected
  subtree. Public encoding emits v5; decoding accepts v4/v5; v4 bytes and
  rejection order remain pinned. V5 must authenticate the exact selected-plan
  payload as well as semantic identities because duplicated caller/callee call
  plans are validator-bound rather than fully selected-identity-bound.
- [ ] Add generated differential testing across interpreter/reference native
  execution for exact integer, float, trap, atomic, placed-memory, cleanup, and
  transition cases.
- [ ] Add end-to-end mutation tests for every manifest and custody field.

## P3 — Psi optimizer

- [x] Control-flow cleanup, SCCP, copy propagation, local/dominating/
  phi-translated GVN, dead pure scalar elimination, and proof-check elision.
- [x] Exhaustive exact rule partitions for wrapping neutral/shift/multiply,
  saturating neutral/multiply, and bitwise neutral/absorbing identities.
- [x] Terminal Psi admits finite cyclic control through SCC topology rather
  than a `Loop` terminator; suspension remains an interprocedural call state.
- [ ] Generalize the exact unsigned-countdown carrier into ordinary cyclic
  execution with fixed-point dominance/frontiers, canonical component IDs,
  well-founded ranking certificates, productive unranked components, and
  structured finite-work failures.
- [ ] Retarget LICM and other loop consumers to validated Terminal SCCs.
- [ ] Implement LICM only after transforms can invalidate and reconstruct
  component, loop-carried custody, and ranking evidence.

## P4 — Lowering and instruction selection

- [x] Mandatory legalization has one ordered declarative catalog for current
  scalar, plain-Unit, and structural-Unit forms; omission and ambiguity fail
  closed.
- [x] Selected construction classifies a complete function body through one
  scalar-family catalog and returns registers and blocks together.
- [x] Exact incoming-u12 add/subtract immediate folds.
- [ ] Add exact wrapping add/subtract/multiply parameter translation families.
  Each owns source grammar, target replay, typed error/receipt, corruption, and
  optimized-custody leaves while sharing only whole-roster ABI/provenance
  mechanics.
- [ ] Add exact address-mode folding, compare/branch selection, extension
  elimination, and constant materialization one named family at a time.
- [ ] Validate ABI operands, calls, clobbers, effects, traps, provenance, and
  logical fuel across every selected rule.

## P5 — Register allocation and frame assignment

- [x] Selected-CFG liveness, live ranges, register views/units/aliases,
  availability, allocation legality, and deterministic transition-free
  interference allocation.
- [x] Fixed-view-copy and active-resident rematerialization recovery with one
  generic allocation-recovery publication carrier.
- [x] x86-64 and AArch64 register-environment ABI/call-clobber corruption
  matrices.
- [ ] Add spill choice, insertion, reload/store validation, and stack-slot
  coloring.
- [ ] Add coalescing, live-range splitting, fixed/precolored intervals, and
  rematerialization cost decisions.
- [ ] Implement frame layout, alignment, red-zone/shadow-space, unwind,
  probing, stable-address loans, and dynamic-allocation constraints.
- [ ] Extend call-clobber validation through general scalar calls and
  live-across-call allocation after calls enter the selected CFG.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic plan/effects with independent
  validation.
- [x] AArch64 CBNZ fusion and MOVN materialization; x86 XOR-zero and
  MOV-r32-imm32 materialization; x86 rel8 layout relaxation.
- [x] Generic encoding/layout/realization carriers let a new substitution add
  one rule leaf and catalog row rather than a new vertical pipeline.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling where independently verifiable.
- [ ] Add target cost models as non-authoritative identities. Semantic
  validation must not depend on estimates.

## P7 — Proof-, ownership-, and state-aware optimization

- [x] Accepted-obligation identities authorize exact proof-check and scalar
  rewrites and remain in the publication chain.
- [ ] Alias/borrow-aware load forwarding, dead-store elimination, and mutation
  motion.
- [ ] Field/variant relevance and invariant-window specialization.
- [ ] Cleanup and transition reachability pruning.
- [ ] State-argument/result specialization with edge provenance.
- [ ] Interprocedural service/call summaries and proof-bound inlining.
- [ ] Proof-directed loop bounds, induction simplification, and vectorization
  with exact lane semantics.

Every rule names the exact proof, ownership, effect, and provenance facts it
consumes and retains their identities through publication.

## P8 — Search and ML extensibility

- [x] Versioned, pointer-free input/output schemas expose exact source, target,
  selection, rule, candidate, fact, and cost-model identities.
- [x] Record-only mode cannot change output; deterministic replay rejects stale
  or mismatched contexts and candidates.
- [ ] Add a sandboxed external policy boundary with timeout/resource limits
  and an explicit fallback.
- [ ] Add offline corpus capture, training, evaluation, and regression tools.

ML may rank already-declared equal transformations. It cannot invent an
unchecked rewrite or opt into lossy floating-point semantics.

## P9 — Verification, stabilization, and rollout

- [>] Complete per-rule positive, negative, boundary, disabled, budget,
  determinism, idempotence, and corruption coverage.
- [x] Cross-rule phase-composition matrix, including fail-closed unsupported
  combinations.
- [ ] Add randomized valid-Psi and selected-machine differential corpora.
- [ ] Add supported target/OS allocator, encoding, unwind, object, and callable
  matrices.
- [ ] Add versioned compile-time, memory, code-size, and runtime benchmarks.
- [ ] Publish exact-rule release notes and rollback procedures.
- [ ] Require owner-reviewed promotion criteria per exact rule; never promote
  an implicit broad level.
