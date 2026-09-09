# Register allocation

This stage consumes the current selected program and its validated analysis
evidence, assigns physical homes, and performs admitted pressure recovery.
Start at [lib.rs](src/lib.rs) and
[stage_register_allocation](src/assignment/current.rs). The preceding selected
X-to-X stage owns selected rewrites and reusable selected-CFG analyses; allocation
does not rerun that phase. The [optimization overview](../../../optimization.md)
identifies the compiler sequence.

## Current facts and evidence

[RetainedAllocation](src/output/mod.rs) owns the current program separately from
its source and transformation evidence. `AllocationSource::replay_allocation`
reconstructs that evidence before exposing `AllocationOutput`. Baseline, literal
fold, fixed-view-copy, rematerialization, and executable-spill histories share
one downstream allocation view. Projection or hashing a raw plan is not admission.

Selected liveness/range schemas belong to `selected-instructions`; allocation
legality, home, and pressure schemas belong to `register-homes`. Algorithms,
errors, and validated receipts stay with the transform that establishes them.
Register views, storage/write units, fixed operands, ties, early clobbers,
reservations, and ABI call effects come from the validated target register
environment. Fixed operands constrain particular occurrences; they do not
preassign every value in the program. Aliasing follows register units, not
register names or modulo scratch-register numbering.

The two liveness domains remain distinct: virtual values use
`before = uses union (after minus defs)`, while architectural units use
`before = implicit_uses union (after minus (implicit_defs union clobbers))`.
The selected analyses number before/after points as `2p` and `2p + 1`, retain
maximal block-local half-open fragments, and connect exact successor transfers;
they do not replace these with convex whole-function intervals. Candidate views
exclude reservation, liveness, and action storage/write conflicts. An exact fixed
view need not belong to the generally allocatable set.

## Home assignment and recovery

[Home assignment](src/assignment/home_assignment/mod.rs) independently validates
the transition-free, spill-free constraint solution. Tied values share a domain
formed by intersecting legal views. Interference uses storage/write conflicts;
early-clobber conflicts are directional. The deterministic producer selects the
most constrained viable domain, then constrained degree, earliest live point,
and canonical value order, and chooses the lowest compatible view. Replay
reconstructs domains, conflicts, and placement separately. Exhaustion is typed
pressure, not permission to manufacture a home or silently change policy.

The producer prepares domain-pair interference, both directional early-clobber
relations, and candidate view lookups once per allocation attempt. These private
facts avoid repeating source scans during greedy placement; they do not change
candidate order, ranking, error order, or the independently reconstructed result.

Selection prepares scalar edge copies as ordinary instructions in explicit
implementation blocks. Only the final transfer register and destination parameter
must share a home; original arguments and snapshots remain independently live.
The allocator retains its interference and tied-home checks. Recovery that
requires an authored node must match a `Source` block origin, not the semantic
target anchor of an edge-copy block.

Fixed-use recovery separates factual intervals, incompatible fixed-use splits,
and segment homes from the decision to insert a copy. Different fixed views
may alias; a split requirement alone prescribes no physical movement. The
[recovery entrance](src/assignment/recovery.rs) consumes exact selected policy
and validated prerequisites. A changed selected graph requires fresh liveness,
ranges, and legality before its homes are accepted. Unsupported ties,
early-clobber cases, use/definition patterns, or graph shapes reject at the
owning analysis rather than falling back to a weaker recipe.

The leaf-local fixed-use copy policy preflights the complete work budget before
any insertion, assigns fresh dense instruction IDs, and preserves original
return fuel/provenance. A zero-copy result is identity. Fixed-mismatch facts
alone grant neither moves nor homes; all post-rewrite analyses bind the
transformed identity and are independently reconstructed.

Without an optional recovery selection, genuine `NoCompatibleHome` pressure
can enter [runtime spill recovery](src/assignment/runtime_spill/mod.rs). It
visits a finite roster of original instruction-result values, restricted to the
failing function and values interfering with its failed register. The selected
rewrite owner admits ordinary nonaddress fixed 8/16/32/64-bit integer, Boolean,
and GPR-resident IEEE
payloads in a single returning block, preserving exact scalar type and
source-definition lineage, storing once, and reloading at
each flexible use. Rewritten values and spill addresses never expand the
candidate roster. Each cumulative rewrite gets fresh liveness, ranges,
legality, and homes; independent replay reconstructs each pressure failure,
rewrite, and final fact/manifest join. The current allocation view remains the
same downstream representation. The manifest records exact transformed
identities and realized selected storage, not final frame authority.
These private eight-byte slots preserve full GPR payloads. They do not widen
source referent reads, change scalar signedness, or normalize floating bits.

The [assignment group](src/assignment/mod.rs) also exposes compiler-private
logical spill, slot-coloring, recursive-recovery, pseudo, and access-constraint
boundaries. These are not additional user-selectable optimizations. Each
boundary binds its input roots, closed policy, exact work and budget, and
independently reconstructed output. Typed original-value and reload-action
lineage must survive recursive store/reload/rewrite schedules. A reload identity
is not automatically a selected virtual register.

Slot coloring uses explicit logical lifetimes and spill-area-relative geometry;
touching closed endpoints conflict. Logical store/reload schedules, chosen
reload views, and abstract read/write effects do not establish executable
addresses, instruction semantics, or final frame layout. Within-block order and
overlapping abstract spill slices imply neither cross-block execution order nor
program-memory alias facts. These bounded APIs do not establish complete
executable spilling through native publication.

## Preservation and frame handoff

[Preservation discovery](src/preservation/mod.rs) intersects selected writes
through their assigned views' exact `write_units`, including implicit definitions
and clobbers, with the target ABI's complete callee-saved roster. It retains
ordered witnesses and explicit empty function rows. Independent replay uses
current allocation facts, not a producer-supplied preservation summary.

These are may-write requirements, not save/restore instructions. Target grouping,
storage placement, return-address handling, frame protocol, and final-machine
reconciliation belong to [machine emission](../../backend/machine-emission/README.md).
Abstract spill evidence cannot select an SP/FP base, red-zone policy, probing,
unwind behavior, or a source crash route. The public
[storage contract](../../../../wiki/spec/resources/storage.md) requires final
realization-bound demand and admitted backing before execution.
