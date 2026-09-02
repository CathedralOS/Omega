# Design Brief: Optimizer Architecture

Status: active architecture contract.

Omega exposes optimizations by exact name. It does not expose `O1`, `O2`,
`O3`, `debug`, or `release` aliases. An empty selection is the ordinary build
path and constructs no optimizer machinery. A build opts in with entries such
as:

```omega
builder.optimizations.enable(Optimization::GlobalValueNumbering);
builder.optimizations.enable(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1);
```

The rollout architecture gate rejects broad level, size-profile, and
build-mode spellings if they ever enter the canonical optimization vocabulary;
this is an executable constraint, not naming guidance.

This document is the entrance to the optimizer design. It owns the invariants,
the pipeline, and the source taxonomy. Details descend into the linked briefs;
the execution checklist lives in [`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md).

## Non-negotiable invariants

1. Every optimization has one stable source-visible name, one declared phase,
   one versioned rule identity, and one independently reconstructible validator.
2. The selected set is ordered and identity-bearing. Enabling, disabling, or
   reordering applicable rules changes the selection or rule-set identity.
3. Rules propose atomic plans. They do not partially mutate compiler state.
4. A producer may use cached analyses; acceptance independently reconstructs
   the facts on which the rewrite depends.
5. No optimized bytes acquire publication authority merely because an ISA
   encoder accepted them. Custody is retained through validation, layout,
   realization, object construction, and callable-entry publication.
6. Exact, wrapping, saturating, trapping, fused, and unfused operations are
   distinct semantics. Ambient fast math does not exist.
7. Proof, ownership, borrow, effect, trap, provenance, and logical-fuel facts
   survive until the last stage that can use or must preserve them.
8. Re-running a completed deterministic pass is stable. Work is bounded by an
   explicit budget and deterministic tie-breaking.
9. Release rollback is an exact-name, subtractive overlay. It never mutates
   the authored build selection, enables a rule, or introduces a profile.

## The coordination shape

Omega adopts the useful shape of Squalr's scan planner while tightening its
contracts:

```text
explicit selections
        |
        v
ordered stage catalog -----> immutable analyses
        |                         |
        v                         v
rule proposal ------------> atomic candidate plan
                                  |
                                  v
                     independent reconstruction
                                  |
                         accept / reject + receipt
                                  |
                                  v
                    one typed stage result
```

Squalr makes its built-ins obvious in a small registry, keeps individual rules
in named leaves, maps inputs to an execution plan, and dispatches from that
plan. Omega preserves those strengths. It does not copy Squalr's global unsafe
singleton, hash-map scheduling, in-place partial mutation, or implicit analysis
invalidation.

Exact names must remain visible below the catalog. Generic `rule.rs` or
`rules.rs` leaves and parent-wide glob imports defeat that property even when
the entrance itself is short; migrated families use exact rule directories
with explicit dependencies and keep only genuinely shared mechanics at their
nearest common ancestor.

Each rule-owning optimizer stage therefore has exactly one small, meaningful
entrance. "Small" is necessary but not sufficient: a short re-export wall is
not an entrance, and a thousand-line leaf below a short `mod.rs` is not a
navigable taxonomy.

The entrance:

- the entrance owns the stage input/output and its ordered catalog;
- `analyses/` owns immutable facts and their revision/invalidation rules;
- `rules/<exact-name>/` owns model, identity, proposal, independent replay,
  codec, and focused tests;
- `validation/` owns shared representation and publication checks; and
- broad compiler routes consume one typed stage result rather than branching
  into a parallel pipeline for every rule.

A cross-stage custody boundary also has one small entrance, but it consumes the
rule owner's catalog; it never creates a proxy enable/order table. Thus the
human path is always rule entrance -> adjacent catalog -> named family -> exact
rule leaf, while the pipeline path is custody entrance -> typed dispatch ->
validated stage result. The post-allocation machine route enforces this
literally: composition retains the canonical catalog entry, and execution
dispatches on that entry's closed implementation kind. No downstream file
repeats the `Optimization`-name schedule.

An entrance answers: what enters, which exact rules can run, where their sole
order is declared, what proposal/validation join executes, and what validated
value leaves. A `mod.rs` that only groups neighboring executable boundaries is
a stage-group map, not a stage entrance, and must say so explicitly.

Custody stages use the same navigational rule even when they do not own a rule
catalog. For example, run-to-abstract replay enters through `replay/mod.rs`,
then descends through `candidate_decisions/mod.rs`; that coordinator visibly
orders manifest binding, independent retained-declaration replay, and baseline
evidence before returning to ledger and external-policy validation. The leaves
are named for those responsibilities rather than accumulated in one generic
decision file.

Here, “stage” means an executable transformation or validation boundary, not a
directory used only to group neighboring boundaries. The Psi reference shape
is concrete: `rules/mod.rs` applies exact selections,
`rules/catalog.rs` visibly lists the ordered passes, and each
`rules/passes/<exact-pass>/mod.rs` visibly lists that pass's local rule order
while routing into named mechanics. The executable
architecture test checks those files and the coordination seams of migrated
physical stages. Its own inventory and checks must follow the same taxonomy; a
giant bespoke path list would merely relocate the navigation problem.
Remaining forwarding entrances, oversized semantic leaves, and broad fixtures
are active organization debt, not evidence that the small-file rule alone has
been satisfied.

Within that shape, `control_flow_cleanup/mod.rs` remains the sole local
rule-order entrance. Its `block_merging/mod.rs` child is intentionally a
non-executable family map: adjacent and non-adjacent merging keep separate
stable registrations, contracts, and proposal rows, while descending into
their own accounting leaves, shared exact substitution reconstruction, and a
merge-boundary ownership leaf shared with jump fusion. The subgroup therefore
does not invent a second catalog or hide the parent schedule.

## Pipeline

```text
checked Psi
  -> Terminal Psi optimization
  -> target/legalized operations
  -> selected instructions with virtual registers
  -> liveness, ranges, constraints, and allocation
  -> post-allocation symbolic-machine optimization
  -> layout-independent encoding
  -> function-relative layout and relaxation
  -> whole-function validation
  -> fragment/text/object construction
  -> callable-entry publication
```

The major optimization phases are:

| Phase | Input | Output | Examples |
|---|---|---|---|
| Psi | validated optimization unit | validated transformed unit | CFG cleanup, SCCP, copy propagation, GVN, dead scalar elimination, proof-check elision |
| Selected lowering | selected virtual-register program | validated selected rewrite plan | exact incoming-immediate folds |
| Allocation recovery | selected program plus allocation facts | revalidated physical homes | fixed-view copies, bounded rematerialization |
| Post-allocation machine | physical symbolic instructions plus liveness | validated form-substitution plan | AArch64 CBNZ/MOVN, x86 XOR-zero/MOV-r32-imm32/MOV-r64-imm32 |
| Function-relative layout | encoded rows plus labels | validated resolved layout | x86 rel32-to-rel8 relaxation |

Selections remain exact even when rules share a phase. There are no broad
profiles. Initially the physical pipeline admits only explicitly implemented
compositions and rejects all others.

Mandatory lowering may expose a machine-rule candidate without authorizing the
rewrite. The exact unsigned-`U64` parameter zero-comparison families, for
example, lower their authored zero and equality to `CompareI64Zero` plus
nonzero control; the inequality form also retains its source Boolean-not as
branch provenance. AArch64 still publishes baseline `CMP`/`B.NE` when that
machine rule is absent; only the separately named
`Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1` selection may replace that
pair with `CBNZ`. This distinction lets source coverage grow without silently
promoting a physical optimization.

Psi analysis has one exact cyclic admission rather than a broad loop mode. The
optimizer-only Terminal authority accepts ordinary acyclic input plus the
existing unsigned-countdown ranked SCC. Verified-context validation separately
reconstructs the Terminal/current operation graphs and requires their canonical
component structures to agree. `CycleComponentId` binds the owning machine and
internal edge roster; derived topology retains sorted members, entries, and
exits. The opaque validated carrier freezes the component blocks, gives only
that function a scoped cycle policy, remains attached to the verified optimizer
session, and is rederived after each run. Bare optimization-unit validation
remains acyclic. An adjacent analysis-only certificate independently infers the
same exact unsigned-countdown ranking relation from Terminal and current IR,
then joins header, rank carrier and bounds, positive guard, minus-one backedge,
and subtract-obligation evidence under the component identity. The opaque
certificate is available to optimizer analyses and is publicly
reauthenticatable, but does not generalize cyclic admission. Together these
carriers are sufficient for CFG, dominator, SCC, loop, liveness, and exact
countdown-ranking analysis, but grant no rewrite, execution, interpretation,
native, or publication authority. The first authority-sensitive consumer is a
separate counted-loop analysis entrance: it joins the opaque component and
ranking carriers to the current unit revision, requires one preheader and one
exit, and records the preheader rank argument as a symbolic exact trip count.
Its direct construction and independently keyed replay must agree on the full
certificate, component roster, edges, carrier type, and unit roots. It is
deliberately absent from the generic bare-unit analysis manager because that
entrance cannot supply ranking authority.

The adjacent countdown-invariant-constant entrance is the second
authority-sensitive consumer. It accepts only the two input-free integer
constants named by the authenticated guard-zero and decrement-one certificate
rows. Direct construction locates them by certificate operation identity;
independent replay separately joins component and counted-loop maps and locates
the current nodes by canonical provenance before reconstructing their exact
definition, value/type, provenance, fuel, and effect custody. The result names
the existing preheader only as a prospective destination. It does not move a
node, invalidate a component, or bypass the ranked-cycle frozen-block fence.

The third authority-sensitive consumer turns those two invariant rows into
exact placement facts without granting a transform. Its revision- and
Terminal-bound snapshot retains the unique preheader insertion coordinate
immediately before the entry jump, plus the sole guard-comparison consumer for
zero and descent-subtract consumer for one. Independent replay maps the
authenticated component/countdown/invariant custody and rescans current
provenance, definitions, uses, jump shape, and consumer operations without
calling the CFG, dominator, SCC, loop, use-definition, or effect producers. It
remains outside the generic analysis manager and cannot bypass frozen ranked
blocks; a later relocation boundary must invalidate and reconstruct this
custody before LICM may exist.

Current ranking reconstruction no longer assumes those two constants remain in
their original component blocks. A private resolver accepts each exact
certificate constant either at its original role location or in the sole
preheader's canonical constant suffix before the entry jump, and current/
Terminal ranking comparison remains the admission seam. Reconstruction runs
before a preservation-aware freeze that independently compares normalized
source and current component blocks. Only the authenticated zero/one position
may differ; source provenance and fuel remain byte-exact, while definition,
use, and effect rows are revalidated by ordinary core validation at their new
coordinates. Successful relocation reissues identical component and ranking
custody. This changes no certificate identity, public rewrite API, or execution
authority.

Transformed session rebinding now enters full transformed-unit validation
before it reissues component and ranking custody. The invariant producer and
its independent replay locate each certificate-owned constant across the
function and accept only its original role block or the sole role-ordered
canonical preheader suffix; placement replay independently reconstructs the
same restriction. Counted-loop, invariant, and placement snapshots can
therefore be regenerated against the new unit revision after canonical
relocation. The dedicated countdown relocation boundary now consumes that
custody through an immutable revision-bound proposal, independently reconstructs
the transformed unit during validation, and atomically consumes the verified
session during application. It reruns full transformed validation, rebuilds
counted-loop/invariant/placement custody, and emits one canonical transformation
record whose moved-node rows preserve exact provenance and fuel. Budget failure
publishes nothing, stale revisions reject, partially relocated authenticated
inputs normalize, and the canonical pair is a fixed point. This is exact
certificate-zero/one motion only; it creates no general LICM registry,
profitability policy, or cyclic execution authority.

Allocation recovery is one phase carrier rather than one carrier per exact
rule. A tagged source leaf retains either fixed-view-copy or active-resident
rematerialization proof custody; common encoding, layout, whole-function exit,
fragment, object, and callable stages sit above that taxonomy. Adding another
recovery rule therefore adds a source leaf and catalog disposition, not a new
publication vertical.

The admitted recovery-machine compositions preserve that taxonomy:
active-resident immediate-U64 multi-use rematerialization can feed the exact
AArch64 MOVN leaf, x86 XOR-zero leaf, or either exact x86 imm32 materialization
leaf through the generic post-allocation realization's
`AfterAllocationRecovery` source leaf. The composition entrance names those
four pairs explicitly; all other
recovery-machine pairs still reject. The join retains both phase-selection
roots and independently replays source, machine, encoding, layout, and exit
custody before publication.

Call-aware allocation follows the same authority discipline. A closed
attached-Unit U64 three-call form can enter mandatory legalization, selected
CFG liveness, legality, and home assignment with exact target-owned fixed
views and clobbers. That does not grant machine-call authority. In particular,
a preserved allocation home is not an implemented callee-save protocol, and a
selected call is not an encoded relocation. Machine effects reject the form
until stack, memory, trap, preservation, and relocation semantics have their
own validated carriers.

The first preservation fact is now explicit but still non-authoritative.
`stage_allocated_callee_saved_requirements` consumes authenticated selected
instructions and baseline register homes, intersects every selected
definition's exact physical `write_units` (plus implicit definitions and
clobbers) with the target ABI's full callee-saved roster, and emits ordered
per-unit witnesses. Direct positional derivation and separately keyed replay
must agree on every root, unit, witness, and bounded-work count. This says only
which preserved units may be modified. It does not select save/restore
instructions, allocate frame slots, construct a prologue or epilogue, describe
unwind state, or grant machine/publication authority. Later executable lowering
must reconcile this allocation-visible requirement against final machine
effects before any preservation protocol can be admitted.

The adjacent non-authoritative callee-save storage stage answers only how the
target ABI groups those required units into storage carriers. A target-owned,
identity-bound catalog declares the exact preservation view, full unit image,
size, and alignment for each canonical group. Thus fragmented AMD64 GPR units
coalesce into one carrier, Microsoft XMM6-XMM15 remain distinct 16-byte
carriers, and AArch64 D8-D15 retain only their ABI-preserved low halves.
Positional production and independent keyed replay then assign dense slots at
offsets relative to an abstract callee-save area. The result is deliberately
not a frame layout or a save protocol: it chooses no SP/FP base, final offset,
red-zone/shadow-space use, instruction sequence, unwind/probe state, memory or
fault behavior, encoding, emission, or publication, and it still requires
final-machine clobber reconciliation.

Logical spilling and stack-slot coloring are compiler-private allocation
decisions rather than user-selected optimization rules. The coloring entrance
consumes the independently validated logical-spill carrier and returns a
versioned, independently replayed first-fit plan whose offsets are relative to
an abstract spill-area origin. This grants neither final frame layout nor
machine spill/reload insertion authority, keeping frame, ABI, unwind, and
publication decisions at later explicit boundaries.

Fixed-register pressure now has a separate source-topology requirement
boundary rather than being hidden inside home assignment. The
`fixed_precolored_split_requirements` entrance consumes exact ranges, legality,
and fixed point intervals. Its tiny entrance exposes named `compute` and
`replay` coordinators; each descends through function indexing, source
topology, fixed-cut authentication, domain partitioning, and work accounting.
Direct positional production and keyed replay independently form maximal
source segments with nonempty exact-view intersections. V1 accepts a
one-block range or an exact single-entry fanout in which every later fragment
has one connector directly from the source fragment. Only an incompatible
authenticated fixed `Use` may open a later segment, and that opening records
an `IncompatibleFixedUseDomainBoundaryV1` fact. Disjoint exact-view domains do
not by themselves prove that physical movement is required because distinct
views may alias. The artifact therefore does not choose exact-view
qualification, copy, rematerialization, spill/reload, or another recovery
strategy, and is not a segmented home map or transformed liveness result.
Other CFG shapes, ties, any early-clobber participant, fixed Def/UseDef
transitions, ambiguous fixed cuts, and unexplained domain breaks reject in V1.

The adjacent `fixed_precolored_segment_homes` allocation entrance consumes
that complete partition rather than rediscovering fixed transitions. Its
`MostConstrainedLowestCompatibleViewV1` policy joins only
`IncomingSourceEdgeV1` segments whose candidate intersection remains nonempty;
every incompatible opening starts a distinct allocation domain. It precomputes
segment-local interference from overlapping source intervals authenticated by
the original interference relation, compares symmetric physical
`units + write_units` footprints, then places the most constrained remaining
domain in its lowest viable view. Consequently mutually exclusive fanout leaves
may reuse one physical view while still retaining distinct domain identities.
Direct positional production and keyed replay own separate `domains`,
`conflicts`, `placement`, and exact-work ladders. Pressure is a typed result,
not permission to spill. The plan assigns only source-segment homes; it creates
no copy, VReg, instruction, transformed liveness, or assertion that distinct
views require physical movement. A later recovery decision must choose an
explicit strategy, introduce any new VReg/instruction phases, and rerun the
complete liveness/range/legality chain before executable or publication
authority exists.

Segment-home work is structural rather than implementation-loop accounting.
For `F` functions, `R` source registers, `S` segments, `D` allocation domains,
`Q` unordered domain pairs, `C` candidate-footprint comparisons on live
interfering pairs, and `K` candidate-viability probes across deterministic
most-constrained selection scans, both traversals charge
`{rules: 1+F+R+D, candidates: C+K, validation: 1+F+R+S+Q+C+K+D,
commits: 1+S+D, iterations: 1+F+R+S+Q+K+D}`. This keeps a future internal data
structure change from silently changing the public budget contract.

Two more explicit compiler-private boundaries continue that descent. Abstract
spill insertion joins the validated logical operations and slot coloring into
one independently replayed store/reload/rewrite schedule; it still contains no
selected or machine instruction. Reload-value home assignment then replays the
original allocation prefix, removes the validated victim, derives the logical
reload's complete pointwise legal-view intersection, and chooses the lowest
compatible physical view. Its validator reconstructs a point-indexed event
timeline independently from the producer's sorted linear schedule. Neither
boundary grants a real virtual-register identity, memory effect, frame
address, trap policy, encoding, emission, or publication authority.

The adjacent synthetic reload-value entrance gives each validated reload/home
pair a distinct compiler-private `{epoch, ordinal}` identity. V1 admits only
epoch zero and canonical function/logical-value order; producer traversal and
keyed replay remain separate. This closes the namespace prerequisite without
claiming a real selected `VirtualRegisterId`, instruction, or any later
physical authority.

The bounded spill-recovery worklist is the next small executable entrance. It
starts only when independent reload-home replay reproduces exact
`ReloadPressure`, then emits one epoch-one item retaining the source reload,
machine, block, lifetime, class, complete canonical candidate domain, and
separate trigger/worklist budgets. Its validator reconstructs all work axes
without calling the producer. The artifact chooses no victim or assigned view,
creates no instruction or rewrite, and grants no memory, frame, trap, unwind,
encoding, emission, or publication authority.

The public prerequisite for exercising a later recovery epoch begins with a
distinct mandatory-legalization recipe for
`r + (b + (r + (a + b)))`; production and independent replay retain the same
pressure-bearing graph on x86-64 and AArch64 while appending new identity tags
without renumbering existing V10 forms. A separate scalar-selection family and
independent selected replay now retain its exact nine-register,
twelve-instruction form. Restricting the public allocator to two views carries
that same artifact through every validated allocation/spill entrance and
terminates at the exact typed reload-pressure failure on both supported ISAs.
This proves reachability of the recursive-recovery boundary. The bounded
worklist grants only the epoch-one scheduling identity described above; spill
choice, spill-pseudo, memory, frame, and publication authority remain absent.

The adjacent spill-recovery choice retains the complete epoch-one contender
rosters and deterministically chooses the second victim. A following 39-line
`spill_recovery_actions` entrance turns that validated choice into target-
neutral logical storage, store, later reload, and complete rewrite obligations.
Its producer and independent replay bind the namespace, anchors, victim
contract, roots, and exact work usage separately. It grants no real virtual
register, instruction, slot/offset, memory, frame, trap, unwind, encoding,
emission, or publication authority. The following 28-line
`generalized_spill_insertion` entrance recolors both closed lifetimes through
direct production and independently keyed replay, then publishes one canonical
store/reload/rewrite event schedule relative to an abstract spill-area origin.
Closed endpoint conflicts, both source identities, environment/availability,
optimization-unit/fuel roots, and exact work usage are retained. It grants no
additional physical or semantic authority. The adjacent
`generalized_reload_value_homes` entrance then independently replays both
generalized actions. It retains the successful epoch-zero home and the exact
epoch-one pressure outcome together, including the complete candidate domain
and blocker roster, rather than discarding partial evidence or inventing a
view. Producer and replay use separate sorted-schedule and point-indexed
mechanics. The result still grants no selected VReg, instruction, memory,
frame, trap, unwind, encoding, emission, or publication authority; the retained
pressure is the sole input to a later bounded recovery epoch.

That consumer is the small `generalized_spill_recovery_worklist` entrance.
Its direct producer and separately keyed replay project the retained epoch-one
pressure into one compiler-private epoch-two work identity while preserving
the complete candidate domain and blocker roster. The item is deliberately
distinct from a spill action and a selected VReg. It chooses no victim or home
and grants no instruction, memory, frame, trap, unwind, encoding, emission, or
publication authority; the adjacent bounded choice boundary consumes it.

The adjacent `generalized_spill_recovery_choice` entrance owns that choice
boundary. Its direct traversal and independent keyed replay reconstruct each
retained blocker as a typed resident, prove the exact subset whose individual
removal recovers a candidate view, and rank only that subset by farthest live
end then highest canonical value. The V1 result retains the full resident and
contender rosters plus selected/reclaimed views, but remains evidence only: it
does not evict a value, create a logical spill action or selected register, or
grant memory, frame, trap, unwind, encoding, emission, or publication
authority. A second exact policy retains selected-plan and live-range roots and
admits an original resident to ranking only after separate producer and indexed
replay prove its selected role and flexible post-pressure use suffix. Eligible
originals rank ahead of reloads, then by the same farthest-end/canonical-value
order. The current fixture's original is used at the pressure point, so it is
correctly excluded and the reload remains selected; the boundary therefore
proves eligibility mechanics without minting original-victim action authority.

The adjacent `generalized_spill_recovery_actions` entrance converts exact
reload- or original-victim evidence into epoch-two logical store, reload, and
rewrite obligations. The legacy V1 reload entry keeps its original inputs and
identity encoding. A separate V2 original entry additionally binds selected-plan
and live-range roots, reconstructs the original definition and complete
flexible post-pressure use suffix, and retains a closed `Original(VReg)` versus
`Reload(action)` victim type. Direct traversal and independently keyed replay
must agree before receipt sealing. Its action namespace remains
compiler-private; the result grants no selected register, instruction,
physical slot, address, memory effect, frame, trap, encoding, emission, or
publication authority. The adjacent `recursive_spill_insertion` entrance owns
two exact policies rather than silently widening V1. Its legacy entry integrates
only reload-victim obligations; the separate V2 entry admits only a matching
original victim/store source and records typed
`EpochTwoOriginal { work_item, source_pressure, victim: VReg }` lineage. The V1
signature, behavior, and identity encoding remain unchanged, while V2 has a
distinct identity domain. Producer and independently keyed replay recolor all
closed logical lifetimes, order every store/reload/rewrite, and use a closed
stored-value type so compiler-private reload actions cannot be confused with
original selected VRegs. The public original-victim schedule retains three
slots, eleven events, offsets `0, 8, 0`, and a 16-byte abstract spill area; its
existing pseudo lowering preserves `Original(v5)`. The result still grants no
physical slot/address, instruction, memory effect, frame, trap, unwind,
encoding, emission, or publication authority.

The following `recursive_reload_value_homes` entrance closes the allocator
question for every recursive reload without crossing into memory semantics. It
joins the recursive schedule, recovery actions, prior generalized homes,
selected ranges/legality, and register environment. A sorted producer and
independent point-indexed replay treat stores as lifetime cuts, retain typed
source lineage, reconstruct complete candidate and coexisting-home rosters, and
select the lowest compatible view for each later segment. Both reload- and
original-victim chains close without residual pressure on x86-64 and AArch64.
The artifact grants no instruction, memory effect, frame address, fault,
encoding, emission, or publication authority.

The adjacent `spill_pseudo_instructions/homed` V2 entrance consumes the
byte-stable V1 pseudo artifact plus final recursive homes. It retains V1's
abstract storage, typed sources, order, and rewrite producer while adding the
exact `destination_view` to every reload. A distinct policy, identity, direct
producer, and independently keyed replay prevent this field from silently
widening V1. This is still compiler-private pseudo custody, not selected or
machine instruction, address, memory, frame, trap, encoding, emission, or
publication authority.

The following `abstract_spill_memory_effects` V1 entrance projects those homed
pseudos into exact target-neutral abstract `Read` and `Write` rows. It retains
pseudo/action anchors, typed source or reload result, physical views/class, and
abstract storage geometry, but has no fault field, executable address/base,
frame coordinate, opcode, encoding, emission, or publication type. Direct
production and independently keyed replay must agree before admission. This
boundary is permitted by Q1's explicit scope; only conversion into real memory
operations and frame/probing remains owner-blocked.

The adjacent `abstract_spill_access_constraints` V1 entrance orders only those
compiler-private abstract accesses within each selected block. It retains
canonical placements and emits typed stored-value, declared-before-reload, and
overlapping-relative-slice dependencies. A dense `block_ordinal` is explicitly
not a cross-block execution claim, and an overlapping abstract spill slice is
not a program-memory alias judgment. Direct production and independent replay
agree on all rows and bounded work before custody is issued. The carrier has no
executable operation, address/base, frame coordinate, fault/trap behavior,
opcode, encoding, emission, or publication type, so Q1 remains intact.

Fixed-view-copy insertion has two visible executable boundaries:
`fixed_view_copy/mod.rs` owns the selected-policy producer-to-validator join,
while `fixed_view_copy/validate/mod.rs` independently admits root and
constraint custody, replays work and budget, reconstructs the exact leaf-local
or shared-entry transformation, compares the complete selected result, and
issues the receipt. The validator never calls `compute`.

Fixed-view-copy artifacts use one small version-admitting codec entrance.
Legacy V4 remains decode-only and byte-stable, with no structural-function
roster. V5 introduced the scalar selected-plan leaf and structural subtree;
its decoder remains supported and reconstructs absent call proof/crash rows as
empty. V6 retains those exact rows below the structural call leaf, V8 appends
the ordered-comparison vocabulary, and V9 appends the scalar-call kind plus
callee payload. V10 appends the distinct signed-I64 less-than selected kind and
terminator while preserving V4 through V9 decode. Signature, ABI/calling plan,
declarations, boundary settlements, call, effects, ownership, and return fields
remain explicit, while named envelope and payload leaves authenticate both
semantic identities and exact canonical content. This also closes caller/callee
call-plan fields that independent validation checks but the selected semantic
identity does not fully cover.

Function-relative realization uses the same source-navigation rule for its V9
manifest: `codec/mod.rs` owns magic/version framing and final protocol
admission, while content encoding/decoding, post-allocation tags, target layout,
rendering, errors, and cursor mechanics descend into named leaves. Persisted
custody boundaries are executable entrances, not generic codec buckets.

## Where to enter the source

Do not begin by searching for an optimization name across the pipeline. Start
at one of these coordination files and descend through its visible catalog or
route:

| Question | Entrance |
|---|---|
| How does `build.omg` become one exact selection? | `omega-build-evaluation/src/optimization/mod.rs` -> `vocabulary.rs`, `selection.rs` |
| Where is the sole injected exact-name mapping used by both build preludes? | `omega-compiler/src/pipeline/optimization/build_vocabulary/mod.rs` -> `fragments.rs` |
| What checked selection reaches native compilation after release rollback? | `omega-compiler/src/compiler/optimization/mod.rs` -> `rollback/`, `native_realization.rs` |
| Which Psi optimizations were explicitly requested, and what verified plan leaves? | `omega-optimization-pipeline/src/coordination/psi_optimization/mod.rs` |
| Which physical phase composition runs next? | `omega-optimization-pipeline/src/coordination/physical_pipeline/mod.rs` |
| Which exact Psi passes and local rules are enabled? | `omega-psi-optimizer/src/rules/mod.rs` -> `rules/catalog.rs` -> `passes/<exact-pass>/mod.rs` |
| Which selected-lowering or allocation-recovery rule is enabled? | `omega-regalloc/src/rules/<phase>/mod.rs` -> adjacent `catalog.rs` |
| Which post-allocation machine rule is enabled for the ISA? | `omega-machine-optimizer/src/rules/mod.rs` -> `rules/catalog.rs` -> `<isa>/<exact-rule>/mod.rs` |
| How are recorded policy decisions admitted for offline work? | `omega-optimization-policy-offline/src/corpus/mod.rs` -> `capture.rs`, `validate.rs`, `identity.rs`, `split.rs` |
| Where does the non-authoritative reference policy train and evaluate? | `omega-optimization-policy-offline/src/reference_policy/mod.rs` -> `training/`, `evaluation/`, `codec/` |

```text
source/omega-rust/omega/
  backend/plans/
    omega-program-entry-plan/       # governed optimized semantic entry/wrapper carriers
  build/
    omega-build-evaluation/src/optimization/
                                      # exact vocabulary admission and selection extraction
  compiler/omega-compiler/src/
    pipeline/optimization/            # injected vocabulary and checked handoff
    compiler/optimization/            # admission, rollback, native realization
  representations/
    omega-assigned-target-operations/ # concrete assigned-operation carrier taxonomy
    omega-optimization-core/       # one exact-name descriptor, selections, identities
    omega-optimization-unit/       # complete input model, reconstruction, rewrite custody
    omega-register-model/          # register views, units, aliases, ABI facts
    omega-selected-instructions/   # pre-allocation plan and admitted machine effects
  pipeline/
    omega-psi-to-abstract-operations/
                                      # artifact, optimizer-unit, provider, and lowering entrances
    omega-abstract-operations-to-target-operations/
                                      # settlement, per-result, and lowering-family entrances
    omega-target-operations-to-assigned-target-operations/
                                      # temporary compatibility assignment families
    omega-target-operations-to-selected-instructions/
                                      # legalization and selection stage entrances
    omega-terminal-psi-to-native-artifact/
                                      # settlement, realization, providers, wrapper encoding/object
    optimization/
      omega-psi-optimizer/            # Psi analyses, catalog, rules, pass manager
      omega-optimization-validation/  # independent Psi and unit validation
      omega-regalloc/                 # physical analyses, allocation, recovery rules
      omega-machine-optimizer/        # symbolic-machine analyses, plans, rules
      omega-optimization-pipeline/    # cross-stage custody and compiler routes
  tooling/
    omega-optimization-policy-offline/ # corpus admission, source splits, reference training/evaluation
```

Within `omega-optimization-unit`, `rewrite/model/mod.rs` is the non-executable
vocabulary map over source/provenance sites, scalar witnesses and constant
facts, SCCP snapshots, CFG and scalar patches, and the candidate contract.
`rewrite/candidate/mod.rs` remains the sole construction and admission
entrance; splitting the vocabulary does not create another optimizer stage or
catalog. Primitive identity writers live in the neutral sibling
`rewrite/canonical_encoding.rs`, consumed by both fact identities and the
candidate codec without a model-to-codec dependency cycle.

`omega-optimization-unit/src/construction/mod.rs` is the sole low-level
abstract-plan-to-seed projection entrance. It builds functions in source order,
descends through exact provenance, scalar-dataflow, control-flow, fact, and
structural-custody projections, then recomputes the complete unit identity.
Verified optimizer admission remains at
`omega-psi-to-abstract-operations/src/optimization/mod.rs`; the seed split does
not create a second verifier or catalog.

Target-neutral operation projection follows one visible chain:
`lowering/mod.rs` admits and orders the verified module,
`lowering/machine.rs` selects ordinary or structural machine lowering, and
`lowering/machine/operation/mod.rs` exhaustively routes every ordinary
`OperationKind` into an exact semantic family leaf before performing the sole
abstract-operation append. This nested entrance is projection only; artifact
admission remains above it, and it introduces neither an optimizer catalog nor
a second verification boundary.

Within a crate, follow semantic rungs rather than filename prefixes:

```text
src/
  lib.rs                 # crate responsibility map
  analyses/
    mod.rs               # analysis catalog entrance
    <analysis>/          # model, compute, identity, validation, tests
  costs/
    mod.rs               # non-authoritative target-cost entrance
    {model,identity}.rs  # descriptive vocabulary and stable target binding
  rules/
    mod.rs               # target/family catalog entrance
    catalog.rs           # the only built-in order
    peephole_matching/   # bounded descriptor matcher; no enable/order policy
    <target>/<rule>/     # model, compute, identity, validation, codec, tests
  planning/
    <plan>/              # immutable plan construction and replay
```

The physical pipeline mirrors compiler custody, not individual optimization
names. Rule-specific mechanics must not leak upward into complete-route files.

The second cataloged consumer below that taxonomy is AArch64 same-view-copy
elision. Its sub-40-line rule entrance joins a declarative
`CopyI64; ReturnI64` proposal to independent replay; named codec, computation,
identity, model, pattern, validation, and test rungs sit immediately below it.
Its exact build selection and sole machine-catalog row feed a rule-neutral
pipeline leaf, which retains the validated disposition through physical
publication. No current ordinary lowering emits the exact body-tail pair, so
applied deletion remains proven only at the rule boundary and compiler routing
is a deterministic zero-action path. Fixed-view recovery's different shape is
an explicit typed composition refusal rather than an inferred match.

The third cataloged descriptor consumer is the exact AArch64
`CopyI64; CompareI64Zero` elision. It proves a second closed matcher topology
for adjacent ordinary-body instructions, but no generic pattern AST. Its own
small rule entrance joins bounded descriptor proposal to rule-local independent
replay of physical footprints, liveness, provenance, actions, and revisions.
The shared copy-elision artifact codec and generic downstream carrier retain
the new policy without creating a second enablement schedule or vertical
pipeline.

## Deeper briefs

- [Semantic contract](optimizer/semantic_contract.md): observability, exact
  arithmetic, floats, effects, proofs, ownership, provenance, and fuel.
- [Rule engine](optimizer/rule_engine.md): catalogs, analyses, candidates,
  validation, budgets, reporting, and ML/search boundaries.
- [Physical pipeline](optimizer/physical_pipeline.md): lowering, allocation,
  symbolic machine rules, encoding, layout, and publication custody.
- [Source organization](optimizer/source_organization.md): entrance-file rules,
  folder taxonomy, size guardrails, and tests.
- [Rollout](optimizer/rollout.md): build opt-in, compatibility firewall,
  stabilization, and promotion policy.

## Resolved decisions

- Exact named suites are the only user-facing selection mechanism.
- The native command's repeatable exact-name rollback flag is release tooling,
  not another source selection mechanism; its requested, applied, and
  effective sets remain separately visible.
- Empty selection preserves the non-optimizer path and never constructs the
  optimizer-only verifier carrier, unit, pass manager, or projection.
- Terminal Psi is the first optimization IR; checked-tree shortcuts are not.
- Target selection precedes allocation; physical rewriting follows allocation.
- Register allocation is a constraint problem, not modulo scratch assignment.
- Producer and validator implementations remain independent.
- ML/search may choose among already-declared candidates but cannot bypass
  semantic validation or publication custody.
- Lossy floating-point transformation requires a future separately named
  semantic contract; it is never inferred from an optimization level.

## Open language decisions

Only questions that change Omega language semantics belong in
[`OWNER_QUESTIONS.md`](../../OWNER_QUESTIONS.md). Implementation choices,
compiler heuristics, file organization, and rollout policy belong here or in
the task plan.
