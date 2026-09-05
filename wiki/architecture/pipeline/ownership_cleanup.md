# Pipeline ownership cleanup

[Pipeline architecture](pipeline.md) | [Optimization phases](optimization_phases.md)
| [Execution board](../../../TASKS_OPTIMIZER.md)

Status: unfinished implementation plan, reorganized 2026-09-05.
This replaces the extraction-by-extraction breakdown. Published milestones
belong in Git; this document names the remaining architectural outcomes.

## The big moves

Make the actual pipeline understandable: one public stage sequence, transforms
with clear owners, independently usable current representations, and real
selected Psi optimization before Terminal publication.

There are four work packages. Findings become requirements inside these
packages, not more top-level migrations. Finish whole ownership boundaries,
including removal of their old routes and adapters.

| Big move | Visible finish line |
| --- | --- |
| Consolidate owners (package 2, first) | Unhomed and umbrella crates disappear; pipeline directories contain actual transforms. |
| Unify physical execution (package 1) | One implementation replaces the ordinary/optimized route split without losing supported programs. |
| Clean representation roots (package 3) | Each representation has an obvious starting file and owns current data, not a chain of previous stages. |
| Implement Psi optimization (package 4) | Selected passes actually run before Terminal publication; the phase is not an identity-only placeholder. |

Measure progress by those outcomes, not helper extractions, test counts, or new
documents. The numbered packages below are stable references, not a requirement
to finish every physical feature before deleting misplaced owners.

## 1. Replace competing physical pipelines with one pipeline

**Outcome:** empty and nonempty optimization selections use the same physical
stages and current program types through final native publication.

Start at
`omega-terminal-psi-to-native-artifact/src/realization/physical_stage.rs`.
Its `NativePhysicalStageResult::Assigned | Optimized` still chooses different
assignment/emission implementations. Another common wrapper does not fix this.

- Define the common physical postcondition from both implementations' consumers.
  Separate current program data from authority and historical replay inputs.
- Port the ordinary route's supported behavior into the common implementation:
  ranked countdowns, callbacks and Unit structural-scalar operations included.
  The selected legalizer currently rejects some of these.
- Route empty and selected execution through the same selection, allocation,
  machine optimization, layout and emission stages.
- Delete the alternate assignment/emission route and its obsolete adapters.

**Done when:** optimization history no longer selects a downstream compiler;
empty/nonempty, ordinary, ranked and callback controls exercise the common graph.
Genuine target, program-shape and authority distinctions remain explicit inside
the owning stages. Rejecting previously supported inputs is not convergence.

## 2. Consolidate the crate graph around real transforms

**Outcome:** opening `omega/pipeline/` shows transformations and explicit X-to-X
optimization phases, not an umbrella implementation and unhomed helper packages.

Maintain a complete **keep / merge / move / delete** disposition table for every
Omega and Psi pipeline crate, with its destination owner. The immediate
consolidation targets are below. These are implementation clusters within this
package, not new top-level tasks.

| Current cluster | Required destination |
| --- | --- |
| `omega-optimization-pipeline` | Retire the umbrella implementation. Sequencing goes to compiler/native coordination; computations go to real transforms or backend. Delete the old crate and re-export surface after consumers migrate. |
| `omega-machine-optimizer` + `omega-post-allocation-machine-to-optimized-machine` | One explicit machine-optimization phase, with rule execution and private analyses homed there. |
| `omega-optimization-policy` + `omega-optimization-validation` | Build policy to orchestration, durable vocabulary to representations, reusable validity to semantics, rule-local checks beside their rule. Remove the catch-all packages. |
| Callee-saved requirements, save storage, spill/frame requirements, frame layout and protocol substeps | Consolidate phase-private calculations into allocation/frame owners. Preserve their independent checks without a public crate per calculation. |
| `omega-optimization-run-to-abstract-operations` and adjacent abstract transforms | Home projection by its actual input/output contract; optimization history must not define a public program stage. |
| Object, callable-entry and image work still inside the umbrella | Move whole artifact boundaries to existing backend owners, preserving source admission and custody at their proper boundary. No new helper-crate collection. |
| `psi-generic-instances` | Separate durable instance data from instantiation work and home both; a noun-only package is not a transform merely because lowering uses it. |
| Remaining X-to-Y / X-to-X crates | Audit all of them. Keep genuine vocabulary/invariant boundaries; names alone do not establish correct ownership. |

Any exception needs a concrete independent consumer and invariant, not an
appeal to the existing layout. Extracting another manifest or counter is a
substep, not an architectural milestone.

This replaces the former `REGALLOC-STAGE-CRATES` instruction to create six
crates merely because six stage documents exist. Liveness, live ranges,
legality and recovery may remain named module-level steps within allocation.
Update their stage documents to the actual owner; retain independent checking
and dependency discipline without requiring a Cargo boundary per calculation.

**Done when:** every disposition is implemented; umbrella/catch-all ownership
is gone; coordinators sequence typed phases; remaining crates have clear
contracts. No `pipeline-common` or renamed helper collection replaces the mess.

## 3. Finish current-data ownership and representation organization

**Outcome:** every representation has one named root beside `lib.rs`, exposing
the current program independently of its producer's history.

- Audit all Omega and Psi roots, not only checked trees and Terminal.
- Move durable program schemas out of transforms. Distinguish reusable
  pre-Terminal data, including `LoweredTerminalPsi`, from lowering scratch.
- Make ordinary consumers use current data and explicit policy. Only replay
  traverses retained earlier inputs. Keep necessary proof inputs separately.
- Organize subordinate areas around each representation's actual concepts.
  Do not force a universal places/drops/moves/edges schema, duplicate trees,
  or invent representations just to label optimization.

**Done when:** every root is obvious; production consumers do not recover their
program through stage ancestry; data can outlive producers without gaining
admission authority. Architecture checks enforce ownership, not cosmetic names.

## 4. Make pre-Terminal optimization real

**Outcome:** standalone Psi already contains the result of selected Psi passes.

`psi-checked-trees-to-terminal/src/preterminal_optimization/mod.rs` currently
accepts identity execution and rejects nonempty selections. That is unfinished.

- Port applicable target-neutral rewrites and independent checks before Terminal:
  control-flow cleanup, SCCP, copy propagation, GVN, dead pure scalar elimination
  and proof-check elision.
- Preserve exact selection, semantic/proof identity, ownership, effects,
  qualifications and execution evidence through publication.
- Keep optimization X-to-X unless vocabulary or invariants genuinely change.
- Keep the selected checked-tree phase visible. Product pruning remains tracked
  by `CHECKED-TREE-PRODUCT-PRUNING`, not duplicated here; it runs after checking
  authored code and must not hide invalid source.

**Done when:** applicable nonempty selections execute and pass independent
validation before immutable Terminal publication. A separately authorized
receiving lowerer does not secretly rerun Psi passes.

## Shared rules

| Responsibility | Owner |
| --- | --- |
| Current program data, typed identities and evidence records | Representations |
| Independently reusable program validity and proof | Semantics |
| Transform/rewrite execution and private analyses | Owning transform |
| ISA, ABI, object-format, relocation and encoding mechanics | Backend |
| General arena, graph and encoding primitives | Foundation, when genuinely shared |
| Sequencing, build selections and product policy | Compiler/build orchestration |

Producer and checker may share catalog predicates, input-only validation and
small arithmetic/encoding primitives, not the output-producing decision
procedure whose result must be independently checked.

All optimizations remain exact opt-ins from `build.omg`. Empty selection is
identity in the same stage graph. Serial public stages do not mean branchless
internals. Terminal remains portable to another interpreter/lowerer with its
own authority; original frontend state must not become a hidden requirement.

## Execution order and anti-drift rule

1. Start with package 2's owner map and implement a whole consolidation at a
   time. Move current representation roots with their owners where needed.
   Do not postpone all visible crate cleanup until physical feature parity.
2. Complete package 1 against those owners. Missing selected-control, ABI or
   encoding support is subordinate to deleting the alternate physical route,
   not a new general compiler project.
3. Finish package 3 across both halves, then package 4's real Psi passes.

Before each implementation milestone, name the old route, owner or adapter that
will disappear, or the required behavior that will become real. If neither
changes, justify the work as a bounded prerequisite, not completion.

Keep only one active consolidation or convergence slice. Its working notes
name the destination, obsolete code to remove, required behavior and acceptance
check. A prerequisite that expands beyond that slice requires reprioritization;
do not recursively turn every discovery into another cleanup task. If an owner
cannot be consolidated until a concrete missing behavior exists, name that
dependency and continue an independent big move rather than narrowing the whole
project to that blocker.

The taskboard keeps one integration item linking here. Discoveries stay within
these four packages unless they require an independently needed product feature.
Delete completed tasks. Do not append checkpoint histories or test counts to
the board. Maintain the full finish condition rather than narrowing it to the
next helper that is easy to extract.

## Completion checklist

- [ ] One physical pipeline, including empty/nonempty selections and all
  previously supported authority/program forms; obsolete routes removed.
- [ ] Every pipeline crate disposition implemented; umbrella/catch-all owners
  removed without replacement unhomed helper packages.
- [ ] All representation roots clear and current-data ownership independent
  of producer ancestry.
- [ ] Applicable selected Psi passes execute before Terminal publication.
- [ ] End-to-end controls cover standalone Psi, separately authorized resumed
  lowering, native publication and stale/substituted evidence rejection.

Run focused behavior/corruption controls plus repository gates: formatting,
workspace Clippy, architecture tests, workspace all-target checking, and
`test --workspace --lib --no-fail-fast`. Run applicable native/runtime controls
separately and report unsupported host legs as not run. Preserve artifact bytes
for internal moves; real format changes require coordinated versions and replay.

Completion is the actual graph, ownership and behavior satisfying these checks.
More documentation, package renames or tests covering only identity execution
do not prove it.
