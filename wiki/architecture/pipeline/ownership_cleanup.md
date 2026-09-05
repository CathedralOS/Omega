# Pipeline ownership cleanup

[Pipeline architecture](pipeline.md) | [Optimization phases](optimization_phases.md)
| [Execution board](../../../TASKS_OPTIMIZER.md)

Status: implementation audit and proposed work breakdown, recorded 2026-09-04.
This is the continuation reference for `PIPELINE-PHASE-INTEGRATION`, not a
claim that the target architecture has landed. The audit examined `2e8c31bd52`;
the subsequent update to `d7eb11c68f` changed boundary checking/lowering, not the
ownership findings below. Recheck the named code before implementation.

## Resume contract

Suggested goal: complete sections A-D and their acceptance checks, preserving
standalone Terminal publication, separately authorized native lowering, and
existing evidence-corruption rejection. A renamed folder, common borrowed
view, or identity-only optimization seam does not by itself complete a section.

Start by inspecting the worktree and changes since this audit. Reconcile local
implementation work before continuing; the named boundaries below, rather
than a past checkpoint's test results, determine what remains.

Use the execution board as the index and this document as the rationale and
acceptance reference. Keep implementation checkpoints separate from unresolved
design choices.

## Objective and boundary rules

Make the public pipeline a sequence of meaningful X-to-Y transformations.
Representations own the current program; pipeline stages own transformations
and private working state. Optimization is an explicit, build-selected X-to-X
phase. Empty selection executes the same phase as identity, not another backend.

A new representation is justified by a change in program vocabulary or
invariants, not by which optimization ran. A stage can legitimately branch on
ISA, program shape, or an explicit authority role internally. Serial public
stages do not require branchless implementations or one crate per calculation.

Each program representation has one clearly named root file beside `lib.rs`.
That file defines the current program and leads into concept-owned areas.
Control flow, values, storage, calls, ownership, and proof are useful organizing
concepts, not a mandatory identical directory template for every representation.

## Observed gaps and completion checks

### A. Current programs still depend on old stage histories

In `omega-rust/omega/pipeline/`:

- `omega-optimization-pipeline/src/coordination/physical_pipeline/model.rs`
  exposes one physical result struct, but that struct wraps
  `StagedOptimizedFunctionFragmentEmissionSource`, a seven-way history/shape sum.
- Its `stages/artifacts/function_fragment_emission/source.rs` still projects
  inputs through prior homes, legality, ranges, and selection stages.
- `omega-selected-instructions-to-register-homes/src/output/retained.rs`
  stores five old allocation histories. `current()` provides a common borrowed
  view; it does not yet own one independent current allocated program.

The shared emission computation is real progress: it selects algorithms by
program shape rather than optimization history. The remaining adapters are
migration machinery, not the finished representation boundary.

`omega-register-homes/src/register_homes.rs` now owns the raw physical-home
assignment and its version-6 codec under representations. Its prerequisite
identities are data, not validation authority. `omega-regalloc` still owns the
independent validator and its private admitted wrapper; its analysis schemas
also remain to be separated from computation. This removes one wrong-owner
dependency but does not yet supply a complete allocated-program root.

Next: define representation-owned current allocated and physical program roots,
then have producers construct them directly. Retain transformation evidence
separately with exact bindings. Replay may require prior inputs, but execution
must not recover its current program by traversing those inputs. Do not discard
proof inputs merely to make the ownership graph look smaller.

Acceptance: downstream allocation, layout, and emission APIs consume current
representations and explicit policy/evidence. No production consumer selects
its representation or algorithm by allocation/optimization ancestry. Existing
corruption controls still reject stale, substituted, and mismatched evidence.

### B. The outer ordinary/optimized distinction still selects different outputs

`omega-terminal-psi-to-native-artifact/src/realization/target_stage.rs` retains
`NativeTargetStageResult::{IdentityOrdinary, IdentityRanked, Optimized}` and
different lowering entrances. `realization/model.rs` also retains identity
versus selected optimization continuations. Giving both branches a stage name
has not completed convergence.

Next: converge current target and physical program outputs regardless of empty
or nonempty optimization selections. Keep ranked-program, provider, callback,
and other actual authority distinctions explicit; do not erase them in pursuit
of a common struct.

Acceptance: empty and nonempty selections traverse the same public phase graph
and output nouns. Ordinary and ranked authority is preserved and checked at its
own boundary, not confused with optimization history. Standalone Terminal
publication and resumed lowering by a separate authority remain supported.

### C. The coordinator and stage granularity still have misplaced owners

`omega-optimization-pipeline` contains layout, realization, artifact production,
and broad stage re-exports in addition to coordination. It is not yet a thin
sequence of phase calls. `omega-regalloc`, `omega-machine-optimizer`,
`omega-optimization-policy`, and `omega-optimization-validation` mix or expose
responsibilities that need classification before another directory move.

Candidate internal substeps include callee-saved requirement derivation, save
storage assignment, spill/frame requirement derivation, and frame protocol
construction. Their correctness boundaries need not all be public crate
boundaries. Consolidation must retain the distinct checks and evidence.

Next: after current program outputs exist, assign each calculation to the phase
that consumes it. Move durable data and independent validation to their owners;
reduce orchestration to sequencing. Only keep separate crates where a stable
dependency or independent consumer justifies them.

Acceptance: pipeline packages describe transformations or same-representation
optimizers. No coordinator owns program schemas or emission algorithms; no new
miscellaneous `pipeline-common` package absorbs displaced responsibilities.

### D. Psi phase implementation and representation organization are separate gaps

`psi-checked-trees-to-terminal/src/preterminal_optimization/mod.rs` implements
identity execution but rejects every nonempty selection. The explicit seam is
not implementation of the selected target-neutral optimizations. Its
`LoweredTerminalPsi` input is also defined in the lowering crate's `lib.rs`.

In `omega-rust/psi/representations/`:

- `psi-typed-trees/src/typed_trees.rs` already supplies a recognizable root.
- `psi-checked-trees/src/checked_trees.rs` owns only the `CheckedTrees` root
  and its concept map. Fact definitions live below `checked_trees/facts/`.
- `psi-terminal/src/terminal_module.rs` owns only the `TerminalModule` root
  and its concept map. Structural types, control flow, boundary declarations,
  ownership, proof, observation, and identity have separate subordinate owners.
  The codec binds the complete representation source closure rather than one
  formerly monolithic file. Architecture controls protect these two entrances.

Next: distinguish the reusable pre-Terminal program/product from ephemeral
lowering joins and phase results; place reusable representation data with its
owner. Audit the remaining Psi representations against the same root rule;
the two protected entrances do not establish completion for all Psi. Port
applicable target-neutral passes and independent validators to the pre-Terminal
phase as a separately visible implementation step.

Do not replace `CheckedTrees { typed, facts }` merely because it contains typed
trees: checking adds facts to the same trees, so this is meaningful composition,
not an optimization-history wrapper. Do not duplicate whole trees to satisfy a
cosmetic rule. Likewise, a private transformation receipt need not become a
new public representation crate.

Acceptance: a reader can find each root directly from `lib.rs`; root files map
the representation rather than collect unrelated definitions. Selected Psi
optimizations execute before immutable Terminal publication, with independent
validation and retained execution identity. Folder cleanup alone does not
close the optimization implementation requirement.

## Sharing rules

| Shared responsibility | Owner |
| --- | --- |
| Current program structures, typed IDs, evidence records | Representations |
| Independent program validity and proof checks | Semantics |
| Rewrite execution and private analyses | Owning transform or optimizer |
| ISA, ABI, relocation, encoding details | Backend |
| General arena, graph, and encoding primitives | Foundation, when genuinely shared |
| Phase sequencing and product selection | Compiler/build orchestration |

Repeated graph traversals, canonical identity-encoding plumbing, and exact
stage-context joins are candidates for reuse, not a mandate for a generic
framework. Inspect their contracts before sharing them. In particular,
producer and verifier may deliberately reconstruct the same property
independently. Sharing a rewrite's decision procedure with its checker can
remove that guarantee. Prefer small shared primitives with explicit semantics;
keep rule-specific proof obligations and domain-separated identities explicit.

## Suggested execution order

1. Define the current allocated/physical representation roots and exact evidence
   boundaries; replace ancestry adapters at those boundaries (A).
2. Converge outer empty/nonempty lowering on those outputs (B).
3. Remove umbrella-owned algorithms and consolidate phase-private substeps (C).
4. Clean Psi representation roots and separate reusable pre-Terminal data from
   producer joins (D). This organization work can proceed alongside A-C.
5. Complete applicable nonempty pre-Terminal optimization execution (D), with
   the remaining work kept visible until its behavioral controls pass.

For each checkpoint, run focused behavior and corruption controls plus the
applicable repository gates. Architectural controls should detect durable
program structs returning to pipeline crates and consumers reaching through
history wrappers, without prescribing identical fields across representations.
Preserve artifact bytes where the change is internal. If an actual format
change is necessary, update its version, codec, and replay contract together;
do not silently reinterpret old evidence.

Completion is behavioral and structural, not a lower package count. Unsupported
language or target forms must still fail closed rather than use another backend.
Execution can reference sections A-D and their acceptance checks directly.
Keep the task board as the execution index,
not a second copy of this audit or a log of completed checkpoints.
