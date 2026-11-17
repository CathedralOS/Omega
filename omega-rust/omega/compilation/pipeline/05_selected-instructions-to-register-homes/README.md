# Register allocation

This stage gives each value of the selected-instruction program a physical
home (register view) and recovers from register pressure as the build allows.
Start at [stage_register_allocation](src/register_allocation.rs), which runs the
steps; [lib.rs](src/lib.rs) lists every module and its re-exports. The earlier
selected-to-selected stage owns rewrites and reusable control-flow analyses;
allocation does not rerun it. The [optimization overview](../../../../optimization.md)
gives the stage order.

## One entrance

Omega 04 hands this stage one `SelectedAllocationInput`, whatever it did: the
current program, the analyses Omega 04 already established for it, and the
program's `SelectedOptimizationRecord` (the completed rewrite phase, if any,
and the transformations it applied). The entrance takes the selected
allocation-recovery catalog entry, if any, and gets the legality analysis:
Omega 04's own when it built one (after a rewrite phase), otherwise a new one
under the allocator availability the entry requires (all allocatable views
when nothing is selected). It then runs the procedure the entry's rule names
(plain assignment, fixed-view copies, or pressure rematerialization), each
falling back to runtime spilling under remaining pressure, and admits the
result. The record only enters the allocation manifest as evidence; which
Omega 04 phase ran never chooses a procedure. A selected rule that needs
different allocator availability than the analysis Omega 04 handed over is
refused rather than recomputed or ignored; Omega 04 itself refuses a selection
that combines its phases with each other or with a recovery rule.

## Current facts and evidence

[RetainedAllocation](src/output.rs) holds the current program and the facts
this stage produced for it. Building it retains the manifest, selections and
recovery decision without replaying the producer. On an explicit validator
request, `validate_publication` checks the recovery decision against the declared
selection and compares the manifest with the retained Omega 04 history.
Later stages consume current facts; the native driver's explicit audit owns
replay ([verification placement](../../../../pipeline.md#verification-placement)).
Plain, fixed-view-copy, rematerialization and executable-spill results share
one allocation view.

Liveness and live-range record types live in `selected-instructions`;
legality, home and pressure record types live in `register-homes`. Algorithms,
errors and receipts stay with the transform that produces them. Register views, storage and write units, fixed operands, ties,
early clobbers, reservations and ABI call effects come from the validated
target register description. A fixed operand constrains one occurrence; it does
not preassign the value everywhere. Aliasing follows register units, not
register names or scratch-register numbering.

The two liveness domains stay separate: virtual values use
`before = uses union (after minus defs)`, while architectural units use
`before = implicit_uses union (after minus (implicit_defs union clobbers))`.
The analyses number the points before and after instruction `p` as `2p` and
`2p + 1`, keep the longest half-open fragments within each block, and join them
across exact successor transfers, never one interval per function. Candidate views exclude conflicts with reservations, liveness and
the storage or write units of actions. An exact fixed view need not be in the
generally allocatable set.
Legality keeps spans with a constant candidate set within each source fragment.
Only the edges and actions of architectural unit fragments and the value's
fixed sites change the ordinary domain, so derivation visits only those
boundaries; identities, fixed-use partitioning and home assignment use the
spans without expanding a row per point. Point and candidate receipt counts still
cover the whole domain, with checked arithmetic. Values that
share a boundary reuse the general candidates; fixed constraints and
early-definition checks stay per value. These tables live for one invocation,
not as retained results or a cache.

Live-range analysis replays liveness first and lends that exact input pair to
range verification, so one invocation replays liveness once. The public
raw-plan range validator still replays liveness on every call.

## Home assignment and recovery

[Home assignment](src/home_assignment.rs) solves the constraint problem with
no transitions and no spills. Nothing replays its answer: the default compile
trusts the solver ([verification placement](../../../../pipeline.md#verification-placement)),
and `RegisterHomeReceipt` only ties the plan to its inputs' identities and
counts. Tied values share one domain, the intersection of their legal views.
Interference uses storage and write conflicts; early-clobber conflicts are
directional. The deterministic solver picks the most constrained viable
domain (then constrained degree, earliest live point, canonical value order)
and gives it the lowest compatible legal view, preferring
the view that satisfies the most already-assigned copy partners, then the most
unassigned partners that would pick that view by the same ranking. Running out
of views is a typed pressure error, never licence to invent a home or change
policy.

Once per allocation attempt, the solver precomputes interference between
domain pairs, early-clobber relations in both directions and sparse
hardware-view conflicts. It admits all candidates in the original domain and
view order before placing any; later choices only remove candidates. This
avoids rescans without changing candidate, ranking or error order. Since an assignment only removes views
from remaining domains, the solver keeps its candidate sets and constraint
counts and applies just the newest assignment. Initial
view validation keeps the original error order.
A physical range that is truly empty needs no home. Its semantic register may
stay for exact bindings, but its legality must then also have no occurrences,
fixed claims, transfers, ties or other physical constraints.

Instruction selection emits scalar edge copies as ordinary instructions in
explicit implementation blocks. A live destination parameter and its final
transfer register must share a home; original arguments and snapshots stay
live on their own. The allocator still checks interference and tied homes.
Recovery that needs an authored node must match a `Source` block origin, not the
semantic target anchor of an edge-copy block.

Fixed-use recovery keeps the facts (intervals, incompatible fixed-use splits
and segment homes) apart from the decision to insert a copy. Different fixed
views may alias, and needing a split does not by itself move anything. The
[recovery entrance](src/assignment.rs) takes the exact selected policy and
validated inputs. A changed graph needs fresh liveness, ranges and legality
before its homes are accepted. Unsupported ties, early-clobber cases,
use/definition patterns or graph shapes are refused by the analysis that owns
them, never handled by a weaker fallback.

The leaf-local fixed-use copy policy checks the whole work budget before
inserting anything, assigns fresh dense instruction IDs and keeps the original
return work and provenance. With zero copies the program is unchanged. A fixed
mismatch alone grants neither moves nor homes; every analysis after the rewrite
binds the new program identity and is rebuilt independently.

An empty whole-range domain (`NoCommonCandidate`) takes a separate refinement
inside recovery: an ordinary untied non-address integer fixed use gets one copy
just before its consumer. The original cyclic or edge-carried lifetime stays
intact and the new one-use segment gets its own home. It creates no storage and
picks no victim. Unsupported origins, types and ties are still
refused. New fixed-use segments do not join the pressure roster; every
committed graph gets current liveness, ranges, legality and independently
validated homes. If a refused call-crossing spill proposal exposes such a
conflict, recovery falls back to the bounded proposal before refining.

Without an optional recovery selection, genuine `NoCompatibleHome` pressure can
enter [runtime spill recovery](src/assignment.rs). A declared fixed-view
selection reaches it two more ways. First, the segment-home front end is tried
on the borrowed legality before the sequence commits; if it declines for
capacity (`SegmentPressure` from placement or `BudgetExceeded` from a front-end
derivation), the still-owned legality goes to spill recovery with the declined
policy recorded. Second, `NoCompatibleHome` left after materialized copies
enters through the full post-copy reanalysis, so the copy step precedes every
spill step in one manifest.

Recovery visits a finite roster of original instruction results and block
parameters in the failing function that interfere with the failed domain. A failure names the domain leader, but ties and edge
transfers split that live range into disjoint member fragments sharing one
home. Pressure is counted across those split points, so a victim overlapping
any member fragment, or a member fragment itself, counts. The candidate's own
component counts too: a retired reload can hold a shared home while its
still-rewritable destination lives in another block. Placement and recovery
share one partition into tied and edge components. Interference across the
component decides eligibility; failed members it covers directly decide
priority. Recovery first tries admitted fixed-view splits in order of direct
coverage, then ranks spill victims by direct coverage and actual live extent,
keeping roster order on ties. Indirect peers stay eligible after direct
candidates run out. A tied-domain leader may not be spillable; every candidate
tried still passes ordinary admission and leaves the roster before its attempt.
This private heuristic does not change the allowed spill evidence or
rematerialization policies.

The selected rewrite code admits ordinary non-address fixed 8/16/32/64-bit
integer, Boolean and GPR-resident IEEE payloads whose single defining
instruction dominates their uses, cyclic functions included. Uses in the same
block must follow the definition. An edge-copy snapshot can keep a semantic
block-parameter identity without being a physical entry parameter. A real
block parameter needs a complete join over incoming edges: every predecessor is
a dedicated edge-transfer block that stores the genuine argument right after
its current definition (a copy or an already-admitted reload) before reaching
the destination. That destination must dominate every reload, which keeps the
parameter's exact type and lineage. Entry reachability with the initialization
block removed finds any path that skips the store.
Original bindings and registers stay; the now-unused parameter adds no live
edge tie. Entry parameters store at the actual entry boundary; cyclic
incoming paths must set up the same initialization. Body, terminator and
outgoing-edge uses reload through the same storage operation.
Unsupported incoming paths, and uses reachable without initialization, are
still refused. Only changed blocks' settlement indices move. Rewritten values
and spill addresses never grow the candidate roster; admitted splits can add
their surviving pinned segment. Each cumulative rewrite gets fresh liveness,
ranges, legality and homes, each checked as it is produced; the finished
recovery is not replayed, and later stages see the same allocation view. The
manifest records the exact transformed identities and selected storage, not
final frame authority. These private eight-byte slots hold full GPR payloads;
they do not widen source reads, change scalar signedness or normalize
floating-point bits.

The [assignment group](src/assignment.rs) holds the procedures the entrance runs.
Runtime-spill recovery rewrites physical victims directly; there is no logical
spill plan or stack-slot coloring.

## Preservation and frame handoff

[Preservation discovery](src/preservation.rs) intersects each selected write,
through its assigned view's exact `write_units` (implicit definitions and
clobbers included), with the target ABI's full callee-saved list. It keeps ordered
witnesses and explicit rows for functions that write none. Replay uses
current allocation facts, not a producer summary.

These are may-write requirements, not save and restore instructions. Grouping
for the target, storage placement, return-address handling, frame protocol and
reconciling the final machine code belong to [machine emission](../../backend/machine-emission/README.md).
Abstract spill evidence cannot choose an SP/FP base, red-zone policy, probing,
unwind behavior or a source crash route. The public
[storage contract](../../../../../wiki/spec/resources/storage.md) requires the
final realized demand and admitted backing storage before execution.
