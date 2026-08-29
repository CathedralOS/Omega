# Optimizer Rule Engine

This brief defines the common rule-planning machinery. The architecture
entrance is [optimizer_architecture.md](../optimizer_architecture.md).

## Stage entrance

Each stage entrance owns four things and only four things:

1. the validated input type;
2. the ordered catalog of exact rules applicable to that stage;
3. the deterministic apply/validate loop; and
4. the single validated stage-result type.

The catalog is the obvious enable/disable point. It maps stable
`Optimization` names to rule descriptors in canonical order. A descriptor
declares rule and validator identities, phase, safety class, required analyses,
invalidations, budget axes, and target/feature predicates.

Rule mechanics live under `rules/<target-or-family>/<exact-rule>/` and use the
same lower taxonomy:

```text
mod.rs       # public rule operation and responsibility map
model.rs     # candidate/plan/outcome types
identity.rs  # rule, validator, and result identities
compute.rs   # producer proposal
validate.rs  # independent reconstruction and acceptance
codec.rs     # canonical persistence, when needed
tests.rs     # positive, negative, boundary, and corruption cases
```

`mod.rs` is meaningful: it joins proposal to independent validation. It does
not contain either implementation.

## Analyses

Analyses are immutable products keyed by validated unit revision and declared
dependencies. The initial catalog includes CFG, predecessors/successors,
dominators, postdominators, SCCs, loops, call graph, scalar constants, ranges,
liveness, live ranges, register availability, alias/ownership frontiers,
effects, and target cost facts.

A rule declares what it consumes and invalidates. After commit, the manager
either preserves a product by proof or recomputes it. Tests deliberately lie
about invalidation to ensure stale facts cannot be reused.

## Atomic candidates

A candidate contains:

- source unit and selection identities;
- rule, candidate, and affected-region identities;
- required analysis and typed fact references;
- an immutable patch/plan;
- predicted cost change, if any;
- provenance and fuel mapping; and
- consumed work.

Construction never mutates the source. Validation independently rebuilds the
precondition, applies the plan to a fresh value, validates the result
representation, and creates an identity-bound receipt. Only then may the pass
manager commit the new value.

## Scheduling

The exact selected set is projected by phase without replacing the full
selection identity. A stage walks its catalog order, applies only selected and
applicable rules, and uses deterministic candidate ordering. Fixed-point passes
have explicit convergence and iteration budgets.

Unsupported combinations fail closed. Broad profiles and implicit target
defaults do not silently add rules.

## Validation layers

1. Representation validation reconstructs structural invariants.
2. Rule validation independently reconstructs each rewrite precondition.
3. Stage validation checks accounting, invalidation, provenance, and custody.
4. Translation validation compares source and lowered/machine contracts.
5. Differential tests compare optimized and reference execution where an
   interpreter or executable oracle exists.

A rule cannot call its own producer as its validator.

## Reports and decisions

Machine-readable decision rows bind input, candidate, rule, verdict, consumed
analyses/facts, validator, budget, and usage. Human reports are projections of
those rows, never authoritative inputs.

## Search and ML

An external policy may rank or choose already-declared candidates through a
versioned input/output schema. The request binds source, selections, target,
rule catalog, cost model, and feature identities. The response names candidate
identities and scores or decisions. Missing, malformed, stale, or mismatched
responses fail closed or use an explicitly selected deterministic fallback.

Search and ML never invent unchecked rewrites, mutate semantic contracts,
grant publication authority, or make the baseline compiler depend on a model.
