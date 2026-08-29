# Design Brief: Optimizer Architecture

Status: active architecture contract.

Omega exposes optimizations by exact name. It does not expose `O1`, `O2`,
`O3`, `debug`, or `release` aliases. An empty selection is the ordinary build
path and constructs no optimizer machinery. A build opts in with entries such
as:

```omega
builder.optimizations.enable(Optimization::GlobalValueNumbering);
builder.optimizations.enable(Optimization::X86SelectXorZeroI64MaterializationV1);
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

Each optimizer stage therefore has exactly one small, meaningful entrance:

- the entrance owns the stage input/output and its ordered catalog;
- `analyses/` owns immutable facts and their revision/invalidation rules;
- `rules/<exact-name>/` owns model, identity, proposal, independent replay,
  codec, and focused tests;
- `validation/` owns shared representation and publication checks; and
- broad compiler routes consume one typed stage result rather than branching
  into a parallel pipeline for every rule.

An entrance is not a re-export wall. It answers: what enters, which exact rules
can run, in what order, and what validated value leaves.

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
| Post-allocation machine | physical symbolic instructions plus liveness | validated form-substitution plan | AArch64 CBNZ/MOVN, x86 XOR-zero |
| Function-relative layout | encoded rows plus labels | validated resolved layout | x86 rel32-to-rel8 relaxation |

Selections remain exact even when rules share a phase. There are no broad
profiles. Initially the physical pipeline admits only explicitly implemented
compositions and rejects all others.

## Where to enter the source

```text
source/omega-rust/omega/
  representations/
    omega-optimization-core/       # one exact-name descriptor, selections, identities
    omega-optimization-unit/       # complete input model, reconstruction, rewrite custody
    omega-register-model/          # register views, units, aliases, ABI facts
  pipeline/optimization/
    omega-psi-optimizer/            # Psi analyses, catalog, rules, pass manager
    omega-optimization-validation/  # independent Psi and unit validation
    omega-regalloc/                 # physical analyses, allocation, recovery rules
    omega-machine-optimizer/        # symbolic-machine analyses, plans, rules
    omega-optimization-pipeline/    # cross-stage custody and compiler routes
```

Within a crate, follow semantic rungs rather than filename prefixes:

```text
src/
  lib.rs                 # crate responsibility map
  analyses/
    mod.rs               # analysis catalog entrance
    <analysis>/          # model, compute, identity, validation, tests
  rules/
    mod.rs               # target/family catalog entrance
    catalog.rs           # the only built-in order
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
- Empty selection preserves the non-optimizer path.
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
