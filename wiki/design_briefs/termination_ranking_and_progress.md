# Design Brief: Termination, Ranking, And Progress

Settled 2026-07-18 (frozen decision 23). This brief replaces the old
`terminates { decreases ...; }` split with one source family, separates a
published completion guarantee from its implementation witness, and settles
the v1 boundary representation of positive progress assumptions.

## One source family, two semantic fields

Omega uses one clause family:

```omega
terminates;
terminates by remaining;
terminates by items -> Slice::Length;
terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
```

The normalized representation keeps two different things:

- **termination guarantee**: every invocation reaches a terminal outcome,
  conditional on its authored requirements and pinned callee/provider progress
  premises;
- **ranking witness**: the subjects, ranking view, optional rank range,
  cyclic-component mapping, and checker certificate used to prove a checked
  implementation.

The guarantee is part of a published machine contract. The witness is private
implementation evidence. One source family does not fuse their identities.

Bare `terminates;` is primarily for bodyless requirements and exported
abstractions. It authors eventual terminal progress; it does not promise a
particular return value, fairness, a deadline, no suspension, or no blocking.
An omitted termination clause on a bodyless requirement promises no eventual
terminal outcome.

For checked bodies the guarantee is inferred from an acyclic graph or a valid
ranking for every cycle. A private acyclic machine writes nothing. A cyclic
machine supplies `terminates by ...` because choosing a witness is an authored
act the compiler does not invent.

Inference does not publish a promise. An exported concrete machine that omits
bare `terminates;` publishes no termination guarantee even when its current
body is acyclic. Local direct calls may use the exact checked summary; calls
through a trait, import slot, or exported contract use only the authored or
inherited guarantee. Refactoring a body therefore cannot silently change what
external callers may assume.

An implementation satisfying a requirement inherits the requirement's
guarantee and premises. It does not repeat `terminates;`; a textual
`terminates by ...` on the implementation supplies only the witness needed to
discharge the inherited claim.

## Ranking witnesses

`terminates by subject -> View` selects a well-founded ranking theory and
requires the produced rank to become strictly smaller on every cyclic edge.
The vocabulary is direction-neutral:

```omega
terminates by n -> Nat::Descending;
terminates by index -> Nat::IncreasingTo(limit);
terminates by node -> Tree::ProperSubtree;
terminates by (outer, inner) -> Lexicographic;
```

`IncreasingTo(limit)` is well-founded because the bound is part of the view.
An unbounded `Increasing` view is not a valid ranking. Authors never write
synthetic arithmetic such as `limit - index` merely to appease the checker;
the selected view owns that normalization.

The optional range constrains the **rank produced by the view**:

```omega
terminates by cursor -> Cursor::TowardStart in 0..=capacity;
```

It is a termination fact and allocates no storage. Its lower bound establishes
the well-founded floor.

The short form `terminates by n` is legal only when the carrier declares a
stable canonical default ranking, such as an unsigned integer's descending
naturals. Elaboration immediately records the explicit view. A user-declared
measure is never selected merely because it is the only visible candidate;
adding another declaration must not change existing meaning. The compiler
never invents ranking subjects or heuristically chooses a noncanonical view.

Mutually recursive or mutually cyclic machines use one joint ranking for the
strongly connected component, and every cyclic edge must decrease it. The
exact source spelling for differently shaped participants remains deferred;
the normalized SCC rule is settled.

## Calls, loops, and proof-stratum machines

Ranking applies to every checked cycle, not recursion alone. Explicit
state/transition loops and call cycles use the same well-foundedness rule when
they promise termination.

Runtime recursive call cycles remain tail-position only. The ranking proves
legality; tail position permits constant-stack lowering. A measured non-tail
recursive call is valid in the proof/compile-time stratum and rejected when a
runtime lowering is requested. This is one machine taxonomy with
context-derived eligibility, not a separate proof language.

Structural-subterm descent is an automation tier, not the semantic limit of
proof recursion. For a recursive edge whose next subject is computed (for
example `sub(a, b)`), the selected ranking view emits its ordinary strict
decrease obligation. The normal entailment engine may discharge that
obligation from contracts or explicitly cited lemmas such as `sub_lt`; no new
ranking-citation syntax is introduced. Proof-stratum machines use this same
measured-recursion rule without the runtime tail-position lowering fence.

Productive machines may deliberately run forever. A transition loop that does
not promise termination therefore owes no ranking witness.

## Partial correctness, outcomes, and effects

`ensures` remains partial correctness: **if** a return edge is reached, the
result and state satisfy the proposition. A result domain cannot prove that
the edge is ever reached, because completion classifies executions rather
than values.

`effects` remains an event/service/operational ceiling. Reaching a
`ProcessExit` service may appear in the row; the `Aborted` terminal outcome is
not itself an effect. The checked artifact may derive a completion
classification from:

```text
termination guarantee x reachable terminal outcomes x explicit premises
```

That derived classification adds no phantom `invocation` carrier and no
surface `Completes<...>` syntax.

## Progress premises and trust

An effect row says which events may occur. It does not identify the premise
under which a suspended operation makes progress. Pinned operation and
provider contracts supply those premises and guarantees.

V1 progress profiles are named, opaque semantic domains over boundary-provider
capability values. They use ordinary domain declaration syntax, for example:

```omega
domain Scheduler::WeakFair { introduction sealed; }
```

The qualification is a commitment, not an inferable predicate: it supplies no
operators, never flow-narrows into existence, and does not entail another
profile in v1. Profiles are sealed by default and use the existing boundary
grant machinery:

- only the profile owner or explicit acceptance authority may authorize a
  claim;
- a package cannot self-grant a progress claim;
- imported claims are inert until granted, and trust expenditure is visible
  in receipts and reports;
- profiles participate in deterministic provider/slot admission; and
- opaque profiles never enter the ordinary proof-fact catalog or entail one
  another.

A termination guarantee names an accepted progress profile through the normal
requirement surface, for example `requires scheduler in WeakFair`. The profile
is a sealed semantic qualification of the provider/capability, not a new
machine clause or ambient promise.

The normalized guarantee records the actual pinned premises, not merely the
presence of `Suspend` or `Block` in an effect row. General machine-side trace
propositions, deadlines, starvation freedom, and entailment between progress
profiles remain deferred until a trace logic exists.

## Identity and revalidation firewall

Published contract identity contains the authored termination guarantee,
explicit premises, and terminal/failure contract. It excludes ranking
subjects, selected view, rank range, SCC mapping, and proof certificate.

Changing a provider from `Nat::Descending` to
`Nat::IncreasingTo(limit)` revalidates that provider and changes its proof
cache key. It does not change an import slot, a caller contract ID, or trigger
contract-driven recompilation of dependents. If an API deliberately publishes
a complexity or resource bound, that belongs in a resource/`ensures`
contract, not in the hidden ranking witness.

## Acceptance register

1. A bodyless requirement or export may write `terminates;`; published omission
   makes no eventual completion promise even when a current body derives one.
2. An acyclic checked body derives termination without source annotation.
3. A cyclic implementation uses `terminates by ...`; its witness proves but
   does not redefine an inherited/public guarantee.
4. Runtime non-tail recursion is rejected at lowering while the same measured
   shape is eligible for proof-time evaluation.
5. An increasing cursor is accepted through a bounded ranking view without an
   authored subtraction.
6. Adding a second user measure cannot reinterpret a short-form witness.
7. Every edge in a mutually cyclic component decreases one joint ranking.
8. `terminates` plus `Suspend` remains conditional on the pinned wake/progress
   premises; the effect row alone cannot invent them.
9. An ungranted provider cannot self-assert a sealed progress profile.
10. Swapping a provider's valid ranking witness revalidates that provider only;
    caller and slot contract identities remain unchanged.

## Migration ruling

The old block form and standalone `decreases`/`increases` clauses are retired:

```omega
// retired
terminates { decreases items -> Slice::Length; }

// current
terminates by items -> Slice::Length;
```

The parser, typed and checked trees, proof cache, diagnostics, canaries, core,
standard library, samples, and compiler lattice corpus must migrate as one
deliberate compatibility-breaking pass. Historical decision records may quote
the old spelling when clearly labeled; normative documentation may not.

## Deferred, explicitly

- Source spelling for joint rankings across differently shaped mutual-cycle
  participants.
- General trace propositions and their proof calculus.
- Deadline, starvation-freedom, and quantitative progress contracts.
- Entailment or refinement between progress profiles after they cease to be
  opaque.
