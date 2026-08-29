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

[x] The ordinary no-selection path now bypasses optimizer-only artifact
lowering, optimizer-unit construction, pass management, and optimized-plan
projection. A focused compiler canary runs on every supported host and checks
all four hosted native targets. It pins empty selection/reporting, exact
acceptance and diagnostics, interpreter output, two-build raw native-byte
determinism, and reviewed per-target artifact metadata/digests. UEFI remains
outside this direct-native matrix until its physical adapter/publication chain
exists.

[x] Psi rule coordination now has one small selection/application entrance,
one declarative pass table, and one local ordered `catalog.rs` in every exact
pass folder. Copy propagation no longer survives as a flat catch-all file. The
architecture test rejects a missing pass catalog or a named stage entrance
that regresses into a re-export wall.

[x] Every Omega pipeline root is now governed by the source-navigation gate;
small stage entrances retain their real joins and semantic leaves remain below
the production/test ceilings.

[x] Exact per-rule release rollback is now a typed, native-only, subtractive
overlay. Unknown and duplicate names reject; known unselected rules are visible
no-ops; publication retains the authored/requested/applied/effective receipt;
and rollback-to-empty matches the ordinary path byte-for-byte on all four
hosted native targets.

[x] Target-register-environment corruption coverage now spans System V AMD64,
Microsoft x64, AAPCS64, and Darwin AAPCS64 across all five supported native
targets. General selected-call lowering and live-across-call allocation remain
separate work because calls are not yet present in the selected CFG.

[x] Psi, selected-lowering/allocation-recovery, and post-allocation-machine
rule crates expose meaningful selection entrances immediately above their
ordered catalogs and named leaves. Cross-stage custody code consumes those
catalogs rather than owning proxy enable/order tables; the architecture gate
pins both the entrances and their tables.

[x] One global coverage test composes only the owning catalog views and proves
every `Optimization::ALL` name occurs exactly once in its declared phase. It is
a test, not a second production registry.

[x] Add explicit target-applicability dispositions to rule descriptors so
unsupported targets reject for a named reason rather than relying on leaf
failure.

[x] Mandatory scalar legalization now has one ordered seven-form catalog.
Producer matching and independent replay descend into separate leaves and
share only recipe, shape, and non-authoritative cost data; the former duplicate
decision tables are gone. Unit and structural-Unit catalog coverage remains.

[x] Layout-independent selected-form encoding now has an independent
validation rung. Its small entrance coordinates ordinary rows, structural
rows, and aggregate custody; candidate bytes descend into target-owned
decoders and cannot re-enter producer encoding helpers.

[x] Resolved selected-form layout now has one small construction/admission
entrance and mirrored semantic subtrees. Ordinary construction descends
through policy, canonical order, span planning, row handling, and target
branch encoding; independent validation separately derives those facts and
admits candidate branch bytes only through target decoders. Architecture and
corruption tests pin both the navigation shape and the producer boundary.

[x] Fragment admission now consumes one generic selected-lowering realization
instead of requiring the x86 rel8 rule. Exact add and subtract selected
lowering reach fragment, text, and object-container custody on both supported
ISAs, then object-artifact and callable custody, without rule-specific route
variants; v8 fragment/text manifests retain the generic source kind and exact
selection identity.

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
- [x] Catalog-driven optimizer slices use analyses/planning/rules/stages with
  focused leaf modules and mirrored tests.

## Organization gate

- [x] Replace `post_allocation_machine_optimizations.rs` with a folder whose
  entrance consumes the machine-rule catalog into one typed result; descend to
  `aarch64/{cbnz,movn}` and `x86_64/xor_zero` custody leaves while the adjacent
  machine-rule entrance remains the only enable/order owner.
- [x] Split `post_allocation_selected_form_encoding.rs` by stage model,
  row/structural encoding, machine-rule disposition, identity, and independent
  validation. Its entrance must own the encode/validate join.
- [x] Remove optimization-name-specific variants from complete physical route
  results once the typed post-allocation result can carry them.
- [x] Derive or exhaustively cross-check build vocabulary and report mappings
  against `Optimization::ALL`; no hidden second registry.
- [x] Keep preferred entrance files below 100 lines. Any entrance above 200
  lines needs a documented semantic reason. No optimizer production file may
  exceed the 1,000-line default ceiling unless it is an exact pinned migration
  debt that cannot grow; no pinned debt may exceed 1,300 lines. Dedicated test
  fixtures retain a 1,500-line ceiling. Files may not mix catalog, unrelated
  rule mechanics, validator, codec, and broad fixtures.
- [x] Eliminate the pinned pre-ratchet production leaves by semantic split.
  Live-range validation is complete: its 34-line entrance owns liveness-custody
  replay followed by independent range replay, with receipt projection and
  focused tests below it; the former 1,294-line catch-all is gone. SCCP constant
  evaluation is also complete: its 35-line shared-contract entrance descends
  into boolean rules and an integer subtree for binary operations, exact casts,
  unary operations, and fact lookup; the former 1,276-line leaf is gone and the
  largest replacement is below 750 lines. Rewrite-candidate construction is
  complete too: its 76-line entrance owns decision derivation, common custody,
  exact patch validation, identity encoding, and immutable admission, with
  scalar/control-flow constructors and accessors in named leaves; the former
  1,253-line file is gone and the largest replacement is below 400 lines.
  Straight-line scalar lowering is complete as well: its 50-line entrance owns
  ordered evaluation and terminal sealing, then descends into exhaustive
  routing, integer arithmetic, integer conversion, and exit leaves; the former
  1,238-line file is gone and the largest replacement is below 600 lines.
  Shared conditional-scalar lowering is complete too: its 37-line entrance
  orders direct scalar handling before exhaustive integer routing, with binary
  semantics and shift semantics in separate shared leaves; the former
  1,111-line file is gone and the largest replacement is below 600 lines.
  Live-range computation is complete as well: its 62-line plan coordinator
  descends through function construction, constraints, fragments,
  architectural units, and focused tests; the former 1,071-line mixed file is
  gone and the largest replacement is below 450 lines.
  Psi-to-abstract machine lowering is complete too: its existing 57-line stage
  entrance descends through a 49-line payloadless/structural/ordinary route,
  then ordinary lifecycle, exact operation, and terminator projection leaves;
  the former 1,058-line file is gone and no replacement exceeds 700 lines.
  Abstract-to-target Unit lowering is complete as well: its ordered setup/loop
  now descends into separate boundary-realization and cleanup-return leaves;
  the former 1,034-line file is gone and no replacement exceeds 453 lines.
  The optimization-unit model is complete too: its 57-line aggregate/map owns
  `PsiOptimizationUnit` above graph, proof, range, ownership, and one-time
  attachment leaves; the former 1,023-line file is gone and no replacement
  exceeds 323 lines.
  Fixed-view-copy computation is complete as well: the existing rule entrance
  retains explicit policy selection and compute-to-validation custody, while
  its application loop descends into source preflight, shared-entry policy,
  CFG mutation, and focused tests; the former 1,022-line file is gone and no
  replacement exceeds 278 lines.
  Legalization leaf replay is complete too: its 95-line entrance owns source
  custody, recipe dispatch, return sealing, and edge-fuel replay, then descends
  into exhaustive recipe, exact-arithmetic, immediate, and fuel families; the
  former 1,022-line catch-all is gone and no replacement exceeds 464 lines.
  Liveness validation completes the migration: its 48-line entrance owns root
  custody, scalar replay/comparison, structural roster replay, and receipt
  admission above named constraint, replay, comparison, structural, receipt,
  shared-canonicalization, and test leaves. The former 1,021-line catch-all is
  gone, no replacement exceeds 225 lines, and the exact exception table is
  empty.
- [x] Clear the current production-file size violations by semantic split, not
  line shuffling. Pipeline `whole_function_exit_contract`,
  `resolved_selected_form_layout`, `x86_branch_relaxation`, and
  `function_fragment_emission` are split, as are validator GVN, SCCP, and
  proof-check-elision candidates, Psi semantic analyses, optimization-unit
  model/rewrite/identity ownership, and matching oversized test suites. A
  repository architecture test enforces file and entrance ceilings.
- [x] Split the architecture brief into a real entrance plus semantic, rule
  engine, physical pipeline, source organization, and rollout briefs.
- [x] Compact this file to an executable checklist; detailed design is not a
  task-list responsibility.
- [x] Replace the conditional Psi mega-registry with a declarative pass table;
  each exact pass owns its ordered rule catalog immediately below its folder
  entrance.
- [x] Split the flat optimized ordinary-callable-entry stage into a subfolder;
  its small entrance owns build/replay while model, reconstruction, and codec
  descend into named leaves.
- [x] Split the flat selected-lowering literal-fold stage; the regalloc rule
  entrance owns exact selection projection and catalog order while pipeline
  carriers, execution, replay, schedule receipts, and work accounting descend
  into named leaves.
- [x] Move selected-lowering, allocation-recovery, and post-allocation-machine
  enable/order tables to their rule-owning crate entrances. Remove the proxy
  pipeline catalogs and enforce each meaningful entrance plus adjacent catalog
  in the repository navigation test.
- [x] Move the 800-line pressure-rematerialization fixture suite out of the
  production compute leaf; the exact rule entrance now descends separately to
  compute, identity, model, validation, and tests.
- [x] Move broad liveness and pre-allocation machine-effect codec fixtures out
  of production compute/codec leaves while preserving shared typed fixtures
  for independent validators.
- [x] Move home-assignment compute and fixed-view-copy codec fixtures into
  explicit path-bound sibling leaves without changing their private test scope
  or test names.
- [x] Split target register-environment custody into a small construction and
  validation entrance above explicit target catalog, validated model,
  validation mechanics, and tests.
- [x] Split selected-instruction staging into retained model, construction,
  fixed-input constraint projection, and independent replay leaves; its
  entrance owns environment-to-replayed-result custody.
- [x] Split optimized target-operation lowering into retained model and exact
  source-route leaves; its entrance owns every lowering-to-custody join and
  provider-installation retention.
- [x] Split bounded target-operation assignment into retained model, source
  lowering, assignment construction, and independent custody replay leaves;
  its entrance owns the construction-to-replay admission join.
- [x] Split selected-CFG liveness staging into model, analysis, independent
  replay, and custody projection leaves below one replay-gated entrance.
- [x] Split CFG-aware live-range staging into model, analysis, independent
  replay, and custody projection leaves below one replay-gated entrance.
- [x] Split exact fixed-view-copy recovery into model, materialization,
  independent replay, and custody projection leaves below a source-validated
  and replay-gated entrance.
- [x] Split transformed-selected reanalysis into complete recomputation,
  independent replay, transition invariant, custody, and model leaves below
  one source-validated entrance.
- [x] Split allocation-legality staging into explicit availability policies,
  analysis, independent replay, custody projection, and model leaves; its
  entrance owns policy selection and the shared replay-gated stage join.
- [x] Split baseline and post-copy register-home staging into construction,
  independent validation, custody projection, and model leaves; the shared
  entrance grants each source family custody only after complete replay.
- [x] Split post-fold and complete selected-lowering home staging into model,
  construction, independent validation, manifest projection, and custody
  leaves below one replay-gated entrance.
- [x] Split post-allocation machine analysis by source-route construction,
  replay/custody validation, and sealed plan model; its entrance owns the
  common effects-plus-machine custody join.
- [x] Split active-resident rematerialization into producer computation,
  independent replay validation, custody projection, and model leaves; its
  entrance alone grants stage custody after compute-to-validation replay.
- [x] Split machine-effect staging into an exact ISA catalog, analysis,
  source-route construction, independent replay, custody, and model leaves;
  its entrance replay-gates every supported selected-source lineage.
- [x] Split active-resident selected-form encoding into retained model,
  source-validated construction, custody projection, independent replay, and
  test-support leaves below one replay-gated entrance.
- [x] Split active-resident resolved-layout staging into retained model,
  policy-checked construction, aggregate custody projection, independent
  replay, and test-support leaves below one replay-gated entrance.
- [x] Split structural-Unit function-relative realization into model,
  construction, independent replay, source admission, manifest reconstruction,
  and custody leaves below one replay-gated entrance.
- [x] Split active-resident function-relative realization into model,
  construction, independent replay, source projection, manifest, custody, and
  test-support leaves below one replay-gated entrance.
- [x] Split receiver-free Unit function-relative realization into model,
  construction, independent replay, source admission, manifest reconstruction,
  and custody leaves below one replay-gated entrance.
- [x] Split the flat optimized object-artifact stage; its small entrance owns
  the terminal/object build-and-replay join while model, reconstruction, and
  canonical codec descend into named leaves.
- [x] Split the flat relocation-free object-container stage; its entrance owns
  construction/replay while model, object assembly, manifest codec, and tests
  descend independently.
- [x] Split the pre-physical manifest monolith into retained model, record
  projection, independent join validation, canonical codec/identity, human
  rendering, and focused test leaves below one projection-to-replay entrance.
- [x] Split complete-unit operation contracts into value flow, ordered node
  contracts, service/call, structural-access, claim-transfer, payloadless-case,
  boundary, and scalar-type leaves below one per-node validation entrance.
- [x] Split current-ownership validation into entry model, ordered CFG replay,
  frontier mutation, cleanup, structural-placement, and residual-affine leaves
  below one current-entry reconstruction-to-replay entrance.
- [x] Split complete-unit structural catalogs into ordered type/domain indexing,
  content projection, type declarations, function-local catalogs, witnesses,
  provider specialization, and path-resolution leaves below one catalog join.
- [x] Split independent rewrite accounting into adjacent/non-adjacent merge,
  terminal fusion, dead scalar, proof identity, common-subexpression,
  substitution, and threading leaves below shared custody/substitution contracts.
- [x] Split independent GVN candidate validation into exact rule classification,
  proof admission, expression keys, dominance reconstruction, local/dominating
  elimination, and phi-translated join synthesis below one custody-and-dispatch
  entrance.
- [x] Split independent dead-scalar validation into exact rule classification,
  an exhaustive operation-safety partition, and rewrite replay below one
  custody-and-analysis-contract entrance.
- [x] Split independent redundant-parameter validation into witness replay,
  closed-region observation normalization, outside-region comparison, and
  exhaustive operation rewriting below one custody-and-analysis entrance.
- [x] Split per-function unit validation into CFG, entry/parameter, result,
  structural-root, fact-index, and provenance/fuel/effect leaves below one
  ordered acceptance entrance.
- [x] Split derived operation metadata into dominance/control-flow, declared
  places, scalar values, provenance, successor edges, and ownership leaves
  below one place-and-claim admission entrance.
- [x] Split current-value-range validation into applicability, independent
  reconstruction, canonical proof-goal, and exact interval-algebra leaves
  below one fact-first validation entrance.
- [x] Split optimized abstract-plan projection into receipt/error models,
  initial-unit and ledger custody, identity-bundle checks, manifest replay,
  source custody, and reconstructible shape leaves below one ordered entrance.
- [x] Split verified/transformed optimizer-context validation into immutable
  context projection, seed/fact replay, surviving frontier validation, and
  signature/roster custody below one revision-policy entrance.
- [x] Split complete-unit core validation into canonical identity/fact indexes,
  active/pruned machine and structural/service catalogs, retained affine
  authority, and final entry/frontier checks below one ordered entrance.
- [x] Replace the 5,000-line target-legalization/instruction-selection entrance
  with separate legalization and selection joins; construction, independent
  replay/validation, identities, constraints, structural/scalar families, and
  focused fixtures descend through named leaves, and the architecture gate now
  governs the entire lowering crate.
- [x] Replace the 4,930-line abstract-to-target lowering entrance and
  2,714-line fixture monolith with settlement/function coordination,
  per-result routing, scalar setup/special/conditional/straight-line families,
  structural routes/layout, Unit lowering, boundary settlement, cleanup, and
  mirrored test families. The architecture gate now governs this entire stage.
- [x] Replace the 3,133-line compatibility target-assignment entrance with
  plan and function coordinators, an exhaustive carrier-family router, and
  named cleanup, boundary, Unit, structural, scalar-control, placement,
  expression-frame, typed-expression, and parameter-discovery leaves. The
  architecture gate now governs the entire assignment stage.
- [x] Replace the 2,924-line Terminal-to-abstract entrance with separate
  artifact admission/replay, verified optimizer-unit construction,
  provider-installation custody, and machine-lowering joins. Proof-fact,
  proof-question, ownership-frontier, payloadless, ordinary-machine, and
  structural-machine mechanics descend through named leaves, and the
  architecture gate governs the entire stage.
- [x] Replace the 1,860-line optimized ProgramStorage semantic-wrapper object
  file with one small owning stage entrance above retained models, semantic
  validation, object composition/validation/manifest construction, custody,
  codecs, and focused fixtures. The architecture gate governs this slice and
  requires each real coordination seam to remain visible.
- [x] Replace the 775-line Terminal-Psi-to-native crate entrance and adjacent
  flat wrapper-encoding stage with a crate responsibility map plus source-entry
  settlement, native realization, provider admission, machine emission,
  artifact assembly, diagnostics, encoding projection, and replay leaves. The
  architecture gate now governs the entire crate and its real stage joins.
- [x] Replace remaining flat executable stages and mixed-responsibility files
  with semantic folders whose small `mod.rs` owns the real stage join. Tighten
  the production-file ceiling as each named legacy leaf is removed.

## P0 — Opt-in and compatibility firewall

- [x] Exact named build selections and canonical order.
- [x] Duplicate, unknown, noncanonical, trailing, and old-version rejection.
- [x] Full selection identity retained across phase projections.
- [x] Empty selection preserves ordinary compilation.
- [x] Add golden canaries comparing no-selection source acceptance,
  diagnostics, interpreter output, native bytes, and artifact metadata on every
  supported host/target pair.
- [x] Add an exact per-rule disable/rollback path to release tooling.

## P1 — Shared rule engine and analysis system

- [x] Stable rule contracts, safety classes, required analyses, invalidations,
  budgets, decisions, typed facts, and manifests.
- [x] Deterministic Psi catalog, scheduling, fixed point, and replay.
- [x] Revision-aware analysis cache with stale-analysis tests.
- [x] Add one obvious ordered post-allocation machine catalog at the
  machine-rule entrance and route its typed result through one complete
  physical realization carrier.
- [x] Unify common catalog descriptors without erasing representation-specific
  candidate and validator types.
- [x] Add catalog coverage tests proving every selected name is scheduled once
  or rejected for an explicit phase/target reason.

## P2 — Validation and publication

- [x] Representation and rule-level independent validators.
- [x] Identity-bound decisions, pass records, manifests, and work usage.
- [x] Source-to-optimized Psi projection and lower-stage custody checks.
- [>] Complete translation validation for all lowering and machine rule
  families. Layout-independent baseline, MOVN, XOR-zero, CBNZ dispositions,
  structural-Unit encodings, and resolved function-relative layouts now replay
  independently; remaining lowering and publication routes still need closure.
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
- [>] Generalize legalization into ordered declarative catalogs of target
  forms, constraints, costs, and validators. The seven scalar forms are
  cataloged; plain Unit and structural Unit families remain.
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
- [x] Add x86-64 and AArch64 target-register-environment ABI/call-clobber
  corruption matrices.
- [ ] Extend ABI/call-clobber validation through general selected scalar calls
  and live-across-call allocation after general calls enter the selected CFG.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic machine plan and effects.
- [x] AArch64 CBNZ fusion and MOVN materialization rules.
- [x] x86 rel8 layout relaxation.
- [x] x86 XOR-zero materialization with RFLAGS-dead proof.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling rules where independently verifiable.
- [ ] Add target cost models as non-authoritative identities; semantic
  validation must not depend on cost estimates.
- [x] Generalize whole-function encoding/layout/realization so new form
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

- [x] Repair the external-decision and projection exact division/remainder
  fixtures. Their proof bundles now derive verifier-reconstructed `/ 1`,
  zero-dividend, `% 1`, and signed `% -1` definedness propositions from exact
  constant semantic axioms through checked integer-bound substitution; proof
  admission remains unchanged.
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
3. [x] Split the encoding stage monolith along that taxonomy (the machine stage
   split is complete).
4. [x] Carry the generic result through encoding, layout, realization, and
   artifact custody with exact byte-delta tests.
5. [x] Retain exact build opt-in and direct/selected XOR-zero coverage through
   publication and callable entry.
6. [x] Finish the remaining stage-entrance taxonomy migration and make the
   navigation contract executable for each migrated stage.
7. [x] Add the broader target-register-environment ABI corruption matrix.
8. [x] Move selected-lowering, allocation-recovery, and
   post-allocation-machine catalogs to their rule-owning crate entrances, and
   make the navigation gate enforce those ownership points.
9. [x] Add global exact-name-to-rule-stage disposition coverage.
10. [x] Add exact target-applicability dispositions at the owning catalogs.
11. [>] Finish workspace validation and rollout canaries before promoting any
   rule beyond explicit opt-in.
12. [x] Replace selected-form producer replay with an independent decoder-led
    validation rung and enforce the boundary architecturally.
13. [x] Replace the x86-rel8-only fragment admission carrier with one generic
    selected-lowering carrier; do not add add/subtract route variants.
14. [x] Unify fixed-view-copy and active-resident realization under one generic
    allocation-recovery carrier before extending either publication route.
    The shared carrier now owns tagged source custody, machine plan, generic
    encoding, resolved layout, whole-function exit, v9 realization manifest,
    and fragment/object/callable admission for both exact rules.
15. [x] Make resolved-layout validation independent before claiming complete
    translation validation for those generic publication routes.
