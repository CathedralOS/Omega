# Register allocation

This stage consumes the current selected program and its validated analysis
evidence, assigns physical homes, and performs admitted pressure recovery.
Start at [stage_register_allocation](src/register_allocation.rs), which
sequences the route; [lib.rs](src/lib.rs) maps every module and re-exports
each owner's names explicitly. The preceding selected
X-to-X stage owns selected rewrites and reusable selected-CFG analyses; allocation
does not rerun that phase. The [optimization overview](../../../../optimization.md)
identifies the compiler sequence.

## Current facts and evidence

[RetainedAllocation](src/output/retained.rs) owns the current program and the
facts the route produced for it. Construction projects one staged result,
checks the exercised recovery against the declared allocation-recovery
selection, and drops the staged producer; later stages read the current facts
and do not replay allocation history, because each fact was checked by the
stage that produced it in the same compile
([verification placement](../../../../pipeline.md#verification-placement)).
Baseline, literal fold, fixed-view-copy, rematerialization, and executable-spill histories share
one downstream allocation view.

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
Legality retains constant candidate spans within each source fragment. Only
architectural unit fragment edges/actions and the value's fixed sites can change
the ordinary domain. Derivation visits those boundaries; identities, fixed-use
partitioning and home assignment consume spans without expanding per-phase rows.
Logical point/candidate receipt counts still describe the full covered domain,
with checked arithmetic. Values sharing a boundary reuse general candidates;
fixed constraints and early-definition checks remain individual.
These invocation-local tables are not retained analysis admission or a cache
across changing selected programs.

Live-range analysis replays its liveness prerequisite before computation and
borrows that exact input pair through independent range verification. This
avoids a duplicate prerequisite replay within one invocation; the public
raw-plan range validator still performs fresh liveness replay on every call.

## Home assignment and recovery

[Home assignment](src/assignment/home_assignment/mod.rs) independently validates
the transition-free, spill-free constraint solution. Tied values share a domain
formed by intersecting legal views. Interference uses storage/write conflicts;
early-clobber conflicts are directional. The deterministic producer selects the
most constrained viable domain, then constrained degree, earliest live point,
and canonical value order, and chooses the lowest compatible view — preferring
the view that satisfies the most already-assigned copy partners, then the most
still-unassigned partners that would themselves take that view under their own
satisfied-edge ranking, among those legal candidates. Replay
reconstructs domains, conflicts, and placement separately. Exhaustion is typed
pressure, not permission to manufacture a home or silently change policy.

The producer prepares domain-pair interference, both directional early-clobber
relations, and sparse hardware-view conflicts once per allocation attempt. It
admits all candidates in the original domain/view order before placement; later
choices only remove candidates from those admitted sets. These private
facts avoid repeating source scans during greedy placement; they do not change
candidate order, ranking, error order, or the independently reconstructed result.
Each new assignment can only remove viable views from remaining domains.
Producer and replay retain their own candidate sets and remaining constraint
counts, applying only the newest assignment instead of rescanning earlier ones.
Replay reconstructs its relationships from current ranges, not the producer's
prepared facts. Initial view validation keeps the original error order.
Genuinely empty physical ranges need no home: selected semantic registers can
remain for exact bindings, but empty legality must also have no occurrences,
fixed claims, transfers, ties, or other physical constraints. Independent home
replay checks the precise resulting roster before preservation or emission.

Selection prepares scalar edge copies as ordinary instructions in explicit
implementation blocks. A live destination parameter and its final transfer
register must share a home; original arguments and snapshots remain independently
live.
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

An empty whole-range domain (`NoCommonCandidate`) follows a separate constraint
refinement inside recovery: an ordinary untied nonaddress integer fixed use gets
one copy immediately before its consumer. This keeps the original cyclic or
edge-carried lifetime intact and gives the fresh one-use segment its own home.
It creates no storage and does not choose a pressure victim. Unsupported origins,
types and ties still reject. Fresh fixed-use segments do not join the pressure
roster; every committed graph receives current liveness, ranges, legality and
independently validated homes. A rejected call-crossing spill proposal that
exposes such a conflict falls back to the bounded proposal before refinement.

Without an optional recovery selection, genuine `NoCompatibleHome` pressure
can enter [runtime spill recovery](src/assignment/runtime_spill/mod.rs). A
declared fixed-view selection reaches the same recovery through two more
arms: the segment-home front-end is probed on the borrowed legality before
the sequence commits, so a capacity decline — `SegmentPressure` from
placement or `BudgetExceeded` from a front-end derivation — hands the
still-owned legality to spill recovery with the declined policy recorded;
and residual `NoCompatibleHome`
after materialized copies enters through the complete post-copy reanalysis,
keeping the copy transformation ahead of every spill step in one manifest. It
visits a finite roster of original instruction-result and block-parameter values,
restricted to the failing function and values interfering with the failed
domain. A failure names the domain leader, but ties and edge transfers split
that live range into disjoint member fragments sharing one home; pressure
evidence is coalesced across those split points, so a victim overlapping any
member fragment — or a member fragment itself — counts. The candidate's own
component participates too: a retired reload can hold a shared home while its
still-rewritable destination lives in another block. Placement and recovery use
one tied/edge component partition. Component-wide interference determines
eligibility; directly covered failed members determine priority. Recovery first
tries admitted fixed-view splits in direct-coverage order, then ranks spill
victims by direct coverage and actual live extent, preserving roster order for
equal scores. Indirect peers remain eligible when direct candidates are exhausted. A
tied-domain leader may not be spillable; every attempted
candidate still passes ordinary admission and is removed from the roster before
that attempt. This private heuristic does not change the permitted spill evidence
or optional rematerialization policies.
The selected rewrite owner admits ordinary nonaddress fixed
8/16/32/64-bit integer, Boolean, and GPR-resident IEEE payloads with one
instruction definition dominating their uses, cyclic functions included.
Same-block uses must follow the definition. An edge-copy snapshot can
retain a semantic block-parameter identity without being a physical entry
parameter. Actual block parameters require a separate complete incoming-edge
join: every predecessor is a dedicated edge-transfer block whose authentic
argument is stored immediately after its current definition (a copy or an
already-admitted reload), before reaching the
destination. That destination must dominate every reload, which retains the
parameter's exact type and lineage. Entry reachability with the initialization
block removed detects any path that could bypass the store.
Original bindings and registers remain intact; the now-unused parameter no
longer contributes a live edge tie. Entry parameters store at their actual
entry boundary; cyclic incoming paths must establish the same exact
initialization. Body, terminator and outgoing-edge uses reload through the same
storage operation. Unsupported incoming paths and uses reachable without
initialization still reject. Only changed blocks' settlement indices
move; edges and other blocks remain identical. Rewritten values and spill
addresses never expand the candidate roster; admitted splits can contribute
their surviving pinned segment. Each cumulative rewrite gets fresh
liveness, ranges, legality, and homes, each checked by its owner as it is
produced; the completed recovery is not replayed. The current allocation
view remains the same downstream representation. The manifest records exact transformed
identities and realized selected storage, not final frame authority.
These private eight-byte slots preserve full GPR payloads. They do not widen
source referent reads, change scalar signedness, or normalize floating bits.

The [assignment group](src/assignment/mod.rs) holds the stages that route
sequences. Runtime-spill recovery selects and rewrites physical victims
directly; there is no separate logical spill plan or stack-slot coloring.

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
[storage contract](../../../../../wiki/spec/resources/storage.md) requires final
realization-bound demand and admitted backing before execution.
