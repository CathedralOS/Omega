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

Allocation recovery is one phase carrier rather than one carrier per exact
rule. A tagged source leaf retains either fixed-view-copy or active-resident
rematerialization proof custody; common encoding, layout, whole-function exit,
fragment, object, and callable stages sit above that taxonomy. Adding another
recovery rule therefore adds a source leaf and catalog disposition, not a new
publication vertical.

The admitted recovery-machine compositions preserve that taxonomy:
active-resident immediate-U64 multi-use rematerialization can feed either exact
x86 imm32 materialization leaf through the generic post-allocation
realization's `AfterAllocationRecovery` source leaf. The composition entrance
names those two pairs explicitly; all other recovery-machine pairs still
reject. The join retains both phase-selection roots and independently replays
source, machine, encoding, layout, and exit custody before publication.

Logical spilling and stack-slot coloring are compiler-private allocation
decisions rather than user-selected optimization rules. The coloring entrance
consumes the independently validated logical-spill carrier and returns a
versioned, independently replayed first-fit plan whose offsets are relative to
an abstract spill-area origin. This grants neither final frame layout nor
machine spill/reload insertion authority, keeping frame, ABI, unwind, and
publication decisions at later explicit boundaries.

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
empty. V6 retains those exact rows below the structural call leaf. Signature,
ABI/calling plan, declarations, boundary settlements, call, effects,
ownership, and return fields remain explicit, while named envelope and payload
leaves authenticate both semantic identities and exact canonical content. This
also closes caller/callee call-plan fields that independent validation checks
but the selected semantic identity does not fully cover.

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
