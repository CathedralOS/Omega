# Cycle checking after provider selection

Ratified 2026-11-12 as `applied-provider-cycle-selection`. Independently check
each implementation's body and contracts, then check provider cycles using the
actual selected wiring. Eligibility for a requirement does not create a call
edge to every implementation that could satisfy it.

The rules live in [synchronous invocation](../../spec/language/effects.md#direct-synchronous-invocation)
and [provider selection](../../spec/build/provider_selection.md#build-owned-target-selection).
This deliberately replaces the former effects rule rejecting cycles among
providers that only a hypothetical future Build could combine. Apply the same
policy to generic and nongeneric providers, not a special case for one sample.

Remaining delivery belongs to **ENTRY-CONTENT-ROOTS** in
[TASKS.md](../../../TASKS.md). This document records semantic acceptance and
migration work, not a compiler implementation or fresh execution evidence.
It changes no source spelling, fixture, diagnostic pin or corpus ledger.

## Independent source checking

Every supplied implementation must satisfy its types, ownership, call
preconditions, effects and authored/inherited guarantees for every admitted
application. Unselected code does not become unchecked. A generic body's
requirement call is retained with exact static arguments and symbolic context:

```text
FirstStage<Phase>::read -> requirement Stage<2>::read
```

No source-level provider lookup expands that requirement dependency into all
eligible suppliers. An independently checked library may retain unresolved
dependencies for its consumer to bind. Direct calls to named machines remain
concrete edges and obey ordinary recursion rules regardless of Build.

## Selected composition

Resolve provider-controlled edges with the selected provider for the exact
requirement application and target. The two configurations have different graphs:

```text
Stage<1> -> FirstStage<1> -> Stage<2> -> SecondStage<2> -> return
    accept: no selected provider cycle

Stage<2> -> FirstStage<2> -> Stage<2> -> ...
    reject: selected provider self-reentry
```

`Stage<1>` and `Stage<2>` are different slots. Neither generic application nor
target identity may be erased to form a graph keyed only by a declaration name.
Adding an unused eligible supplier alone does not change a selected call target.

The graph includes every possible synchronous call admitted by the contracts,
including helpers, callbacks, runtime alternatives and native envelopes. A
successful execution trace is insufficient coverage. Unknown/abstract summaries
retain their status and conservative bounds; they are never empty by default.
Executable composition resolves required supply and checks `invokes` coverage,
acyclicity and existing resource, stack, authority, trust and lifecycle admission.
Replacing a provider reruns the affected checks before accepting its wiring.

## Termination and migration

Selected provider dependency cycles remain prohibited without an authored
`terminates` promise. This settlement does not change that composition rule.
Ordinary permitted tail recursion is not automatically a provider dependency
cycle, and an acyclic provider graph alone does not prove an internal algorithm
terminates. Keep those different obligations under the
[termination rules](../../spec/language/termination.md#calls-and-productive-loops).

Every public or inherited promise still needs justified progress evidence.
Any admitted provider-dependent premise must remain represented until discharged;
selection cannot silently strengthen an abstract contract or make two providers
justify each other circularly.

The existing negative
`fail/termination/unselected_implementation_reentry_rejected` mixes two concerns:
an unselected alternative enters a ranked `walk`, and that `walk` calls a
requirement with no authored termination guarantee. Do not mechanically flip
the whole fixture to accepted. Separate candidate-set graph construction from
the actual progress-contract question, preserving a rejection control for each
genuinely unproved promise. Any resulting fixture split must obey the repository's
existing frozen-facts and new-source limits.

## Acceptance

- The existing
  `pass/capabilities/selected_applied_synchronous_chain_exit` must accept with
  `FirstStage<1>` and `SecondStage<2>` selected and return 73 under native/Psi
  agreement on its declared hosted targets. No extra termination annotation is
  needed merely to eliminate the invented candidate edge.
- Changing the `Stage<2>` selection to `FirstStage<2>` rejects the selected
  self-reentry cycle, including when neither side authors `terminates`.
  Retain cross-provider cycle controls as well as direct self-reentry.
- Adding an unused eligible generic or nongeneric provider must not insert
  edges into the selected product. Invalid source bodies and false authored
  guarantees still reject independently of whether that provider is selected.
- Distinct closed applications and target selections retain distinct graph
  identities; test forwarding through helpers and callbacks without collapsing
  them to the owning trait or machine declaration.
- Libraries retain symbolic requirement dependencies before Build selection;
  executable composition rejects missing, ambiguous, cyclic, cross-target or
  unadmitted demanded supply. Direct calls remain concrete and ordinary legal
  recursion keeps its existing treatment.
- Every possible callback/runtime target permitted by the admitted envelope
  contributes to the cycle check. An opaque native envelope or unavailable
  summary cannot become an empty graph; one benign observed run cannot justify
  another possible path.
- Provider replacement invalidates and revalidates the affected closure and
  `invokes`/resource/admission checks. A formerly acyclic choice cannot certify
  a cyclic replacement. Portable source and artifact consumption agree on the
  exact selected applications and retained call summaries.
- Separate the old ranked negative by its actual contract and wiring intent.
  Do not remove termination requirements, weaken ranked obligations, add an
  ambient guarantee to the boundary, or infer a circular proof merely to make
  an expected result green.

No runtime guard, provider-selection keyword, new annotation or universal
enumeration of hypothetical consumer configurations is introduced. Implementation
and execution evidence remain required before claiming this capability delivered.
