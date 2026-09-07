# Pipeline ownership cleanup

[Pipeline architecture](pipeline.md) | [Optimization phases](optimization_phases.md)
| [Execution board](../../../TASKS_OPTIMIZER.md)

This is a replacement plan, not a commitment to migrate the existing code.
Get to the required architecture, then build out its supported behavior. Code
generation is cheap compared with prolonged adaptation of the wrong structure.
Delete and rebuild an obstructive implementation instead of preserving it through
wrappers, compatibility exports, special cases, and more intermediate stages.

## Preservation rule: behavior, not implementation

Preserve the language contract, working program behavior, target ABI, ownership,
proof, effects, resource bounds, and independently checked publication. Do not
preserve internal APIs, crate boundaries, example-specific IRs, recursive backend
payloads, receipt arrangements, or implementation-shaped tests merely because
they exist. Existing documentation describes constraints only where those
constraints remain justified by the intended architecture and real consumers.

Salvage code when it fits the destination directly and is cheaper to verify than
its replacement: for example, an ISA encoder, ABI fact table, or independent
checker. Otherwise salvage its useful rules and test cases, not its structure.
Neither the old assigned backend nor the current selected backend is privileged
as the template for the replacement.

Temporary breakage is acceptable in an isolated implementation worktree. Do not
keep both designs compiling at every intermediate step by adding adapters that
the final design does not need. Use Git to retain the previous implementation
for comparison. Land coherent, verified checkpoints under the normal publication
rules; do not disguise missing behavior as completed cleanup.

## The four required outcomes

| Move | Visible finish line |
| --- | --- |
| 1. Consolidate pipeline owners | Umbrella and helper crates disappear. Each remaining pipeline crate owns a real transform or explicit optimization phase. |
| 2. Finish representation ownership | Every Omega and Psi representation has one obvious root and usable current data, independent of producer history. |
| 3. Delete the alternate physical pipeline | Empty and nonempty optimization selections use the same physical stages through native publication. |
| 4. Deliver optimized portable Psi | Selected target-neutral passes actually execute before Terminal publication. |

These are finish conditions, not four serial queues. The immediate priority is
the common executable route in move 3. Rebuild its representations and owners
alongside it; do not postpone convergence until every existing representation or
helper has been tidied. Finish the remaining Omega/Psi ownership sweep and
pre-Terminal optimization against that architecture. Directory renames and
private helper extractions are not substitutes for an executable replacement.

## 1. Consolidate pipeline owners

Opening `omega/pipeline/` or `psi/pipeline/` should explain what transforms
the compiler performs. A name containing `to` is not sufficient justification
for a crate, and a named internal calculation need not be a public stage.
The folder sequence must be followable as `X-to-Y`, `Y-to-Y`, `Y-to-Z`.
Optimization uses literal `X-to-X` names, not an `-optimizer` exception.
Competing Psi entrances, orphan outputs and competing selected-instruction
successors remain defects until the actual program route is connected;
renaming their crates does not satisfy this acceptance condition.

The layout sequence separates baseline construction in
`selected-form-encoding-to-resolved-layout` from optional relaxation in
`resolved-layout-to-resolved-layout`. Empty and selected layout phases expose
one raw `ResolvedMachineLayout`; only their checked stage entrances establish
custody. Retained raw data must not act as an unchecked baseline admission or
require a different downstream representation. This boundary is part of the
connected route, not acceptance of the remaining owner and physical-route work.

| Whole move | What must disappear or change |
| --- | --- |
| Connect the visible stage sequence | Keep `terminal-psi-to-abstract-operations` as the Omega program entrance. Consolidate remaining supporting calculations under their owning program stages without introducing alternate downstream representations. |
| Finish the remaining crate disposition | For every other Omega and Psi pipeline crate, decide keep, merge, move or delete and implement that decision. Preserve genuine representation/invariant boundaries, not the existing package count. |

Keep only the disposition information needed to execute the current replacement.
An exception needs a concrete independent consumer and invariant. Do not replace
removed crates with another unhomed helper collection, or turn the inventory
into a second backlog.

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

Start at `omega-rust/omega/compiler/native-realization/src/realization/physical_stage.rs`:
`NativePhysicalStageResult::Assigned | Optimized` still selects competing
assignment/emission implementations.

Define the destination's current program and stage contracts first. Implement
them directly, replacing either existing route where necessary. Empty selection
is identity execution within that sequence, not a different compiler. A wrapper
around both implementations, or a third path beside them, is not convergence.

The replacement must have:

- Ordinary ordered instructions and control edges, with explicit value,
  argument, result, call, memory, relocation, and semantic identities. Delete
  example-specific call forms and admission paths that obstruct this model;
  do not grow a new recipe for each combination of type, caller, and arity.
- Explicit ABI transport for supported scalar widths and signedness, Boolean
  values, register and stack arguments, results, and call clobbers. An existing
  register-only U64 slice is a test case, not the architecture.
- One owner of complete frame planning: local and spill storage, saved registers,
  incoming/outgoing stack arguments, alignment, and required ABI areas such as
  Windows shadow space. Stack accesses retain their actual role and effects;
  ABI argument storage is not disguised as allocator spill storage.
- A deliberate stage for resolving frame references before their bytes are
  emitted. Scratch preservation, argument snapshots, copy scheduling, and frame
  geometry are planned data, not decisions hidden inside byte emission.
- Independent validation of the replacement's semantics and resource claims.
  Reusing an old producer to certify its replacement is not independent replay.

Existing selected and post-allocation roots may be reused or reshaped; their
current fields and schemas are not requirements. Recursive assigned expressions
and complete assigned Unit bodies must not survive as opaque executable payloads
inside the new graph. Unmatched instructions may remain unchanged by a selected
rewrite; they may not skip common allocation, effect validation, layout or
encoding.

Preserve ordinary, ranked-countdown, callback and Unit structural-scalar
behavior through selection, allocation, machine optimization, layout and
emission. Reimplement this behavior in coherent families, then delete their
superseded algorithms, carriers, adapters, and obsolete structural tests. The
previous route can serve as a temporary comparison oracle from Git; it must not
become a permanent fallback. Required call, control, or ABI support belongs to
this replacement, not to an indefinitely deferred prerequisite board. Unrelated
new language/backend features remain outside this cleanup.

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

- Work on one coherent replacement at a time. Name its destination contract,
  the supported behavior it must preserve, the old owners/routes to delete, and
  the acceptance checks. Then implement it; do not repeatedly redesign the plan.
- Prefer replacing a whole obstructive subsystem to a sequence of compatibility
  repairs. If keeping it requires more special cases or ancestry-dependent
  plumbing, stop preserving it and rewrite the owner. Engineering difficulty is
  not a reason to retreat to another cosmetic extraction.
- Helpers, guards, file moves, and codec changes are implementation steps, not
  standalone success criteria. A checkpoint must deliver a real route/behavior
  or remove an obsolete owner; it must reduce remaining convergence work rather
  than create another permanent mechanism.
- Replace tests that assert obsolete implementation shapes. Preserve their
  semantic coverage, negative controls, and independent evidence checks. Do not
  delete a failing behavior test simply to declare the replacement complete.
- Keep discoveries under these four moves. Add a separate product task only for
  independently required functionality, not another cleanup substep.
- Remove completed work. Keep history and test counts in commits, not this plan.
  The taskboard carries one integration item linking here.

A milestone must demonstrate the changed ownership/route and preserved behavior,
with focused controls, formatting, affected-crate Clippy and all-target checks,
architecture tests and relevant integration coverage under the repository's
validation-scope rules. Full-workspace runs are for baseline work or an impact
that cannot be bounded. Run applicable native controls separately and name host
legs not run. Preserve artifact bytes for genuinely mechanical internal moves.
Do not distort a replacement to preserve an obsolete internal wire shape: revise
the format and its consumers together, with explicit versions, stale-format
rejection, and independent replay. Machine-code byte equality is not required
when a valid replacement realizes the same program differently; semantic, ABI,
effect, ownership, and resource correctness remain required.

The cleanup is finished only when all four acceptance conditions hold, including
standalone Psi, separately authorized resumed lowering, native publication and
rejection of stale/substituted evidence. Renames, wrappers, added documentation
and identity-only tests do not establish that result.
