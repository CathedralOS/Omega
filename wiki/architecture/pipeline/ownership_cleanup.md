# Pipeline ownership cleanup

[Pipeline architecture](pipeline.md) | [Optimization phases](optimization_phases.md)
| [Execution board](../../../TASKS_OPTIMIZER.md)

Status: implementation audit and proposed work breakdown, recorded 2026-09-04.
This is the continuation reference for `PIPELINE-PHASE-INTEGRATION`, not a
claim that the target architecture has landed. The original audit examined
`2e8c31bd52`; the current reference includes the allocation, physical-instruction,
and machine-code ownership changes. Recheck the named code and incoming changes
before implementation; these changes do not establish completion of sections A-D.

## Discussion summary

The X-to-Y design is achievable. The remaining problem is not the spelling of
stage names: some outputs still carry earlier stage objects, some coordinators
still own algorithms, and empty optimization selections still take different
routes. Finish those ownership changes before treating folder cleanup as done.

The allocation and fragment-emission boundaries now own current program data
independently of replay history. Earlier realization producers and the outer
native entrance still need their ownership and convergence audit. Psi's
checked and Terminal roots have been organized, but the other representation
entrances and actual nonempty pre-Terminal optimization remain separate work.

Keep these distinctions throughout the cleanup:

- A representation is the current program, not a record of how it was built.
- A transform changes that program; optimization is an explicit X-to-X phase.
- Evidence can retain earlier inputs without making ordinary consumers walk
  those inputs to find the current program.
- Shared mechanisms belong with their actual data, semantic, or target owner;
  repetition alone does not justify a new common package.
- Concept-owned subfolders should explain each representation. Do not force
  every representation into an identical places/drops/moves/edges template.

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

This document is a work reference, not an instruction to start an autonomous
goal. A later goal can name sections A-D directly. The remaining implementation
choices are the physical program's exact data boundary, which phase-private
calculations should be consolidated, and which existing target-neutral passes
can be moved before Terminal with their independent checks intact. Decide those
from their consumers and invariants rather than preselecting crate names.

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

## Goal-ready scope

Use this document as the cross-reference for the pipeline discussion, not as
authorization to begin implementation. A subsequent goal can be stated as:

> Complete ownership cleanup A-D: current-data outputs, one public phase graph,
> transform-owned algorithms, and single-entry Psi representations with real
> opt-in pre-Terminal optimization. Preserve independent replay and separately
> authorized lowering of a published Psi artifact.

The architectural amendments to carry into that work are:

- **Serial stages, not a branchless compiler.** X-to-Y is achievable at public
  representation boundaries. Target, program-shape, and authority cases can
  branch inside their owning transform. Optimization history must not choose
  an alternative downstream compiler.
- **Explicit optimization phases, unchanged representation vocabulary.** All
  optimizations remain exact opt-ins from `build.omg`. Empty selection is the
  phase's identity operation. Do not invent a new representation merely to
  distinguish optimized from unoptimized data.
- **Psi optimization precedes publication.** Terminal means the published
  portable product, not the mutable input to another hidden Psi pass. Additional
  early phases need a concrete benefit and preserved checking obligations;
  checked-tree pruning must not hide invalid authored code.
- **One entrance per representation, not one universal representation.** Put
  the named root beside `lib.rs`, with subordinate areas explaining that
  representation's actual concepts. Stable semantic links across stages are
  useful; forcing identical fields or folders across all stages is not.
- **Share mechanisms, not accidental coupling.** Before extracting repeated
  code, identify its consumers, invariant, and lowest legitimate owner using
  the sharing table below. Do not make producer and verifier share the very
  decision procedure whose result is supposed to be independently checked.

Before implementing each boundary, write down its input noun, output noun,
policy inputs, current-data owner, and separately retained replay inputs. This
is a review check, not a request for a generic stage framework. The unresolved
choices are the exact emission data boundary (A), common outputs preserving
authority roles (B), genuinely reusable calculations versus phase-private
substeps (C), and applicable early Psi passes and their data owner (D).

The finish condition is the completion checklist below, not a directory rename
or package-count reduction. Keep those choices and acceptance checks here;
the task board should only link to them.

## Observed gaps and completion checks

### A. Current programs still depend on old stage histories

In `omega-rust/omega/pipeline/`:

- `omega-optimization-pipeline/src/coordination/physical_pipeline/model.rs`
  exposes one physical result whose emission source retains current data and
  a separate seven-role replay input graph. Its current accessors do not walk
  that graph.
- `stages/artifacts/function_fragment_emission/current.rs` retains current
  program data, admitted machine/layout facts, encoding, frame protocol, exit
  contract, manifests, and the exact target/proof input. `replay.rs` alone
  recovers the earlier producer inputs for independent checking.
- `omega-selected-instructions-to-register-homes/src/output/retained.rs`
  owns a current `AllocatedProgram` plus a separate five-role replay input graph.
  `current()` reads the current admitted facts directly; only replay traverses
  earlier allocation stages. Producers share immutable selected/home artifacts
  with that output rather than copying the selected program into a snapshot.

Fragment emission and structural placement select algorithms by program shape,
not optimization history. Frame application reads the retained current protocol,
not the earlier fixed-frame realization. This boundary does not establish that
all earlier producer-stage packaging has been removed.

`omega-register-homes/src/register_homes.rs` owns the current allocated-program
root; its storage area owns the physical-home table with the unchanged version-6
codec. Its prerequisite identities are data, not validation authority.
`omega-regalloc` still owns the independent validator and its private admitted
wrappers; its analysis schemas remain to be separated from computation. The
allocation admission capsule still retains an upstream target/proof input for
downstream proof joins. That is not a replacement for converging target outputs.

`omega-physical-instructions/src/physical_instructions.rs` owns the physical
instruction program, with subordinate control-flow, instruction, operand,
identity, and codec areas. Its version-5 frame and version-6 content identity
are unchanged. Construction and independent admission remain outside the
representation; selected-form encoding imports the raw data from this owner.

Next: audit earlier realization producers and remove old producer-stage packaging
where only replay inputs are required. Retain transformation evidence
separately with exact bindings. Replay may require prior inputs, but execution
must not recover its current program by traversing those inputs. Do not discard
proof inputs merely to make the ownership graph look smaller.

Raw x86 structural-call footprint and fixup records live in
`omega-machine-code/src/machine_code/calls/structural.rs`;
ISA encoding, decoding, and private validated wrappers stay in the backend.
`omega-machine-code/src/machine_code/layout.rs` owns the current
`ResolvedMachineLayout`, its function/block spans, branch facts, structural-call
fixups, policy, and unchanged version-9 content identity. Its admitted pipeline
wrapper shares the same immutable program. Retaining that data does not retain
the wrapper or grant admission; a separate entrance independently rechecks the
selected/machine inputs, byte decoding, offsets, and optimization records.
Layout-independent encoding identity and internal-call fixup data have the same
representation owner. Generic post-allocation optimization records live in
`omega-physical-instructions` evidence; typed rule results stay with their
producer and independent validator.

`omega-machine-code` also owns `ResolvedMachineProgram`: shared selected,
home, effect, physical-machine, and resolved-layout artifacts. The emission
boundary retains those original immutable artifacts without deep-copy snapshots.
Replay first validates its historical inputs, then compares complete current
artifacts and admission facts, not only rehashable IDs. Raw artifacts remain
usable as data after the producer is dropped, without granting publication
authority. Layout and emission algorithms still need their proper transform
owners; separating data from replay does not complete C.

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

The machine-code representation supplies `machine_code.rs` as its
program root, with functions, calls, storage, control flow, ownership, boundary,
provenance, instruction, and fragment areas. Its raw encoding records do not
grant backend admission. This organization does not complete the coordinator
cleanup or establish that all representation roots are done.

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

## Completion checklist

- [ ] A: physical consumers read independently owned current program data;
  replay retains and checks its inputs separately.
- [ ] B: empty and nonempty selections use the same public phase graph and
  output representations, preserving actual authority distinctions.
- [ ] C: coordinators only sequence phases; schemas, algorithms, validation,
  and target details have their appropriate owners.
- [ ] D: each Psi representation has a clear root and coherent subordinate
  owners; reusable pre-Terminal data is separate from producer-only joins.
- [ ] D: applicable selected target-neutral optimizations actually execute
  before Terminal publication and pass independent validation.
- [ ] End-to-end controls cover standalone Psi, separately authorized resumed
  lowering, empty/nonempty selection, and stale/substituted evidence rejection.

These are acceptance checks, not a requirement to introduce six new packages
or six new representations. The more detailed acceptance text in A-D governs.

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
