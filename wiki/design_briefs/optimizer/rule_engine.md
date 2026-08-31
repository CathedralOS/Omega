# Optimizer Rule Engine

This brief defines the common rule-planning machinery. The architecture
entrance is [optimizer_architecture.md](../optimizer_architecture.md).

## Rule-stage entrance

Each rule-stage entrance owns four things and only four things:

1. the validated input type;
2. the ordered catalog of exact rules applicable to that stage;
3. the deterministic apply/validate loop; and
4. the single validated stage-result type.

The catalog is the obvious enable/disable point. It maps stable
`Optimization` names to rule descriptors in canonical order. A descriptor
has one target-independent header, `OptimizationCatalogDescriptor<Payload>`,
and a stage-owned typed payload. The header preserves the exact source name;
the payload preserves the representation-specific rule/validator identities,
policy, analysis requirements, invalidations, budget axes, and applicability.
This common shape does not make the optimization core depend on target or
representation crates.

Target-independent stages say so explicitly in their payloads. Target-specific
catalogs carry the canonical `omega_target::Architecture` beside each exact
name. Selection checks that predicate before dispatch and returns a typed error
containing the optimization, required architecture, and actual architecture.
Rule-leaf validation repeats the target check as defense in depth; leaf failure
is not the primary applicability mechanism.

Pipeline custody entrances consume that selection result and bind it to their
typed input/output carriers. They do not own another ordered list of the same
rules.

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

Two exact rules may share a non-executable semantic family when they consume
the same custody mechanics without sharing one execution point. Adjacent and
non-adjacent block merging follow this pattern: each retains its stable
identity, version, proposal row, and position in the control-flow-cleanup
catalog, while a small family map exposes their separate proposal/accounting
leaves and shared exact substitution reconstruction. Merge-boundary ownership
custody lives at the parent level because jump fusion consumes it too. The
family map neither enables rules nor owns a second order.

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

The composed catalog test enumerates all exact names against Linux x64,
Windows x64, UEFI x64, Linux Arm64, and macOS Arm64. It proves one phase owner
per name and an exhaustive scheduled-or-named-rejection disposition. The test
normalizes the owning descriptor views only for verification; it is not a
production registry.

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

At the run-to-abstract publication boundary, every independently validated
candidate declaration is retained and rebound to the selected pass partition,
complete built-in rule contract, exact input revision, manifest evidence, and
baseline-policy row. Applied rows must additionally bind one transformation
commit; skipped rows must bind none. Manifest analyses/facts and predicted cost
therefore cannot be coordinately rewritten with the external-policy recording
to manufacture custody for either verdict.

## Search and ML

An external policy may rank or choose already-declared candidates through a
versioned input/output schema. The request binds source, selections, target,
rule catalog, cost model, and one canonical row per validated candidate. Schema
v2 exposes only the candidate identity, predicted structural cost delta,
scheduled analysis set, and exact sorted proof/ownership/fact identities. Its
closed response is either `Choose(candidate)` or `Skip(reason)`; it does not
need an opaque `O1`/`O2`/`O3` profile or a model-authored rewrite. Record-only
mode reconstructs those rows independently from decision manifests, while
replay requires an exact context and row match before consuming the decision.
Missing, malformed, stale, or mismatched responses fail closed or use an
explicitly selected deterministic fallback.

Search and ML never invent unchecked rewrites, mutate semantic contracts,
grant publication authority, or make the baseline compiler depend on a model.
