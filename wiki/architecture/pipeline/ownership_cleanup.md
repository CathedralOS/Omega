# Pipeline ownership cleanup

[Pipeline architecture](pipeline.md) | [Optimization phases](optimization_phases.md)
| [Execution board](../../../TASKS_OPTIMIZER.md)

This is the remaining work, not a history of extractions. Complete whole owners
and routes; remove their obsolete implementations. Progress means the repository
and executable pipeline become simpler, not that more small tasks close.

## The four big moves, in execution order

| Move | Visible finish line |
| --- | --- |
| 1. Consolidate pipeline owners | Umbrella and helper crates disappear. Each remaining pipeline crate owns a real transform or explicit optimization phase. |
| 2. Finish representation ownership | Every Omega and Psi representation has one obvious root and usable current data, independent of producer history. |
| 3. Delete the alternate physical pipeline | Empty and nonempty optimization selections use the same physical stages through native publication. |
| 4. Deliver optimized portable Psi | Selected target-neutral passes actually execute before Terminal publication. |

Use the actual program route to order consolidation: make its entrance,
successors and explicit X-to-X phases visible before dismantling another
private helper family. Move representation data with its owner as needed;
then finish the remaining representation sweep. Directory renaming alone does
not close a route or establish physical feature parity.

## 1. Consolidate pipeline owners

Opening `omega/pipeline/` or `psi/pipeline/` should explain what transforms
the compiler performs. A name containing `to` is not sufficient justification
for a crate, and a named internal calculation need not be a public stage.
The folder sequence must be followable as `X-to-Y`, `Y-to-Y`, `Y-to-Z`.
Optimization uses literal `X-to-X` names, not an `-optimizer` exception.
Competing Psi entrances, orphan outputs and competing selected-instruction
successors remain defects until the actual program route is connected;
renaming their crates does not satisfy this acceptance condition.

| Whole move | What must disappear or change |
| --- | --- |
| Connect the visible stage sequence | Keep `terminal-psi-to-abstract-operations` as the Omega program entrance. Consolidate remaining supporting calculations under their owning program stages without introducing alternate downstream representations. |
| Finish the remaining crate disposition | For every other Omega and Psi pipeline crate, decide keep, merge, move or delete and implement that decision. Preserve genuine representation/invariant boundaries, not the existing package count. |

Keep a compact disposition map while executing this move. It is an inventory,
not a second backlog. An exception needs a concrete independent consumer and
invariant. Do not replace removed crates with another unhomed helper collection.

**Acceptance:** the dispositions are implemented, old owners and adapters are
deleted, and coordinators only sequence typed phases. Directory layout and
dependency direction agree with the actual ownership.

## 2. Finish representation ownership

Sweep all Omega and Psi representations, not just the last one touched.

- One named root file beside `lib.rs` defines the current program and provides
  the reader's starting point.
- Subdirectories group the representation's actual concepts: control flow,
  values, storage, calls, ownership or evidence where those concepts exist.
  Do not impose one universal folder template.
- Durable program schemas live in representations, including reusable
  pre-Terminal data such as `LoweredPsi`; transformation scratch
  stays private to its producer.
- Ordinary consumers read current data. Historical inputs needed for replay
  remain explicit evidence, not the route to finding the current program.

**Acceptance:** every root is obvious, public program data can outlive its
producer, and ordinary consumers no longer walk producer ancestry. Moving
files without fixing those dependencies does not complete the move.

## 3. Delete the alternate physical pipeline

Start at `omega/compiler/native-realization/src/realization/physical_stage.rs`:
`NativePhysicalStageResult::Assigned | Optimized` still selects competing
assignment/emission implementations.

Define one physical stage sequence and current program contract, bring across
the supported behavior of both routes, then delete the alternate route.
Empty selection is identity execution within that sequence, not a different
compiler. A wrapper around both implementations is not convergence.

Use the existing selected and post-allocation program roots. Migrate the finite
assigned operation roster into ordered instructions with explicit call ABI,
memory, frame, relocation and semantic records. Move scratch preservation,
argument snapshots and copy scheduling out of byte emission before retargeting
its ISA encoders. Recursive assigned expressions and complete assigned Unit
bodies must not survive as opaque executable payloads in the shared graph.
Unmatched instructions may remain unchanged by a selected rewrite; they may
not skip common allocation, effect validation, layout or encoding.

Preserve ordinary, ranked-countdown, callback and Unit structural-scalar
behavior through selection, allocation, machine optimization, layout and
emission. Missing selected-control or ABI support is a prerequisite inside
this move, not an invitation to start a general backend expansion.

**Acceptance:** empty/nonempty selections and those existing program forms reach
native publication through the common graph. No supported behavior is removed
to make the routes appear unified. Target and authority distinctions remain
explicit inside their proper stages.

## 4. Deliver optimized portable Psi

Complete the selected pre-Terminal rewrites in
`lowered-psi-to-lowered-psi/src/lib.rs`, including proof-context transport for
dead scalar elimination in proof-bearing closures.

Move applicable target-neutral rewrites and independent checks before Terminal:
control-flow cleanup, SCCP, copy propagation, GVN, dead pure scalar elimination
and proof-check elision. Preserve proof, ownership, effects, qualifications and
selected-execution evidence through publication.

Optimization is an explicit X-to-X phase unless vocabulary or invariants
actually change. All passes remain exact opt-ins from `build.omg`.
Checked-tree product pruning stays under `CHECKED-TREE-PRODUCT-PRUNING`;
it runs after checking authored code and must not hide invalid source.

**Acceptance:** applicable nonempty selections execute and independently validate
before immutable Terminal publication. Shipped Psi is usable by a separate
interpreter/lowerer with its own authority and no original frontend state;
that consumer does not secretly finish omitted Psi optimization.

## Ownership rules

| Responsibility | Owner |
| --- | --- |
| Current program data, identities and evidence records | Representations |
| Independently reusable validity and proof | Semantics |
| Transformation, rewrite execution and private analyses | Owning transform |
| ISA, ABI, object, relocation and encoding mechanics | Backend |
| Genuinely shared arena, graph and encoding primitives | Foundation |
| Sequencing, build selections and product policy | Compiler/build orchestration |

Producer and checker may share input predicates and small primitives, not the
output-producing decision procedure that independent replay must check.
A serial public pipeline does not require branchless internal algorithms.

## How to keep this work finite

- Work on one whole consolidation or route convergence at a time. Before
  starting, name the old owner/route to delete or the missing behavior to deliver,
  its destination, and its acceptance check.
- Helpers, guards and codec changes are bounded prerequisites, not milestones.
  If they expand beyond the active move, reassess priority instead of recursively
  adding tasks. A blocker in one owner does not block independent consolidation.
- Keep discoveries under these four moves. Add a separate product task only for
  independently required functionality, not another cleanup substep.
- Remove completed work. Keep history and test counts in commits, not this plan.
  The taskboard carries one integration item linking here.

A milestone must demonstrate the changed ownership/route and preserved behavior,
with focused controls, formatting, affected-crate Clippy and all-target checks,
architecture tests and relevant integration coverage under the repository's
validation-scope rules. Full-workspace runs are for baseline work or an impact
that cannot be bounded. Run applicable native controls separately and name host
legs not run. Preserve artifact bytes
for internal moves; format changes require coordinated versions and replay.

The cleanup is finished only when all four acceptance conditions hold, including
standalone Psi, separately authorized resumed lowering, native publication and
rejection of stale/substituted evidence. Renames, wrappers, added documentation
and identity-only tests do not establish that result.
