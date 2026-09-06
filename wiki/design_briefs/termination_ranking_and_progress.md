# Design Brief: Termination, Ranking, And Progress

Settled 2026-07-18 (frozen decision 23), amended 2026-08-22 for explicit
progress-profile classification and premise attachment. This brief replaces
the old
`terminates { decreases ...; }` split with one source family, separates a
published completion guarantee from its implementation witness, and settles
the initial boundary representation of positive progress assumptions.

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
through a trait requirement or exported contract use only the authored or
inherited guarantee. Refactoring a body therefore cannot silently change what
external callers may assume.

An implementation satisfying a requirement inherits the requirement's
guarantee and premises. It does not repeat `terminates;`; a textual
`terminates by ...` on the implementation supplies only the witness needed to
discharge the inherited claim.

## Ranking witnesses

`terminates by subject -> View` selects a well-founded ranking theory. State
transition cycles require strict descent on each cyclic edge; mutually
recursive calls use the joint-cycle rule below.
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

Checker legality, validation, checked facts, specialization, terminal
eligibility, and snapshots all consume subjects, argumented-view bounds, and
view/range identity from the normalized `RankingWitness`. No parallel surface
may collapse authored guarantees and private witnesses back into one flag.

The diagnostic firewall follows the same rule: checked-stage messages use the
current `terminates by` spelling exclusively, and even the directed rejection
of a retired subtraction subject is reconstructed from the normalized witness,
not from authored compatibility spans.

Every acyclic body records a local `Terminates` summary, including an
unannotated private body. The
normalized machine contract independently records only the authored promise;
omission therefore remains published `NoGuarantee`, so body inference cannot
become interface authorship.

Named transitions remain inside the current activation. A transition back to
entry names the machine symbol while a transition to a subordinate state names
the state symbol; ranking and checked-progress analysis normalize both through
one target-state rule. The edge consumes the local ranking witness rather than
instantiating the machine's still-forming summary as a nested call, while its
actual arguments still correspond exactly to the resolved state's formal
parameters for progress-subject lineage.

The normalized enum, snapshots, and artifact discriminator spell this positive
case `Terminates`. There is no second completion guarantee behind that name.

`05_machine_contracts.json` serializes that split explicitly. Each machine has
an authored `contract` object
(fingerprint, supply, canonical service-reach and operational ceilings,
published termination) and a sibling `implementation` object (checked summary
and private normalized ranking witness). Requirement-binding and component
tooling pin the former without incorporating proof-local material from the
latter.

Mutually recursive machines use one joint ranking for the strongly connected
call component, and every complete cycle through its call graph must decrease
that ranking. A forwarding call may preserve the rank when every cycle still
contains strict descent. Checking only a DFS traversal's discovered cycles is
insufficient: a preserving cross-edge cycle must also reject. The exact source
spelling for differently shaped participants remains deferred; the normalized
joint-cycle rule is settled.

Checked progress for a private mutual component uses that validated joint
ranking without an authored public promise to seed inference. Calls outside
the component still supply their exact progress obligations. Finite sets of
subject-qualified premises and build-bound provider demands propagate through
the component; unknown external progress prevents the checked guarantee.
Growing projected demands are not generalized into a universal qualification
or silently dropped.

The future proof-certificate form preserves that granularity. One recursive
component cites the selected ranking relation and its well-foundedness evidence
once; every intra-component edge carries its own rank-comparison proof, and
the component check establishes descent on every complete cycle. The
well-foundedness citation is not repeated per call, and a local decrease never
stands in for the global fact that the relation admits no infinite descent.
Both kinds of evidence retain provenance, so an admitted custom ranking theory
makes every recursive proof depending on it admission-dependent. Calls outside
the component use ordinary contract application rather than the recursive
rule.

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
example `Nat::saturating_sub(a, b)`), the selected ranking view emits its ordinary strict
decrease obligation. The normal entailment engine may discharge that
obligation from contracts or explicitly cited lemmas such as `sub_lt`; no new
ranking-citation syntax is introduced. Proof-stratum machines use this same
measured-recursion rule without the runtime tail-position lowering fence.

Productive machines may deliberately run forever. A transition loop that does
not promise termination therefore owes no ranking witness.

## Partial correctness, outcomes, and reach

`ensures` remains partial correctness: **if** a return edge is reached, the
result and state satisfy the proposition. A result domain cannot prove that
the edge is ever reached, because completion classifies executions rather
than values.

`reaches` remains a service-reach ceiling. `suspends` and `blocks` remain
independent operational may-ceilings. Reaching a `ProcessExit` service may
appear in the row; the `Aborted` terminal outcome is not itself an effect. The
checked artifact may derive a completion
classification from:

```text
termination guarantee x reachable terminal outcomes x explicit premises
```

That derived classification adds no phantom `invocation` carrier and no
surface `Completes<...>` syntax.

## Progress premises and trust

Neither service reach nor a `suspends` declaration identifies the premise under
which a suspended operation makes progress. Pinned operation and provider
contracts supply those premises and guarantees.

Progress profiles are named, opaque semantic domains over boundary-provider
capability values. Their domain declaration explicitly selects the
compiler-owned `ProgressProfile` classification and names its closed
establishment routes:

```omega
pub boundary trait SchedulerAdmission {
    machine grant_weak_fair(scheduler: SchedulerHandle)
        -> SchedulerHandle in WeakFair;
}

domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant_weak_fair;
```

`ProgressProfile` is not inferred from an empty predicate body, a provider
route, or use by a terminating machine. Only the domain owner may attach this
classification, and one atomic domain has at most one such classification.
Downstream packages cannot add another classification or append establishment
routes. The qualification is routed and predicate-free: it supplies no
predicate or operators, never flow-narrows into existence, and does not entail
another profile. Profile establishment uses the owner-authored boundary
requirements in `established by` and admitted receipts:

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

Premise attachment has three distinct levels:

1. A bodyless requirement or exported checked machine authors its public
   premise schemas in its contract. For a published `terminates` guarantee, a
   `requires subject in Profile` clause whose domain explicitly satisfies
   `ProgressProfile` is such a schema. An exported checked implementation must
   prove that every dependency derived from its body is covered by one of
   those authored schemas; refactoring the body cannot silently rewrite the
   public premise set.
2. A checked call instantiates premises only from the exact selected
   operation's termination contract and its explicit argument substitution.
   Merely receiving, mentioning, suspending with, or forwarding a
   progress-qualified value creates no dependency. Private checked machines
   therefore derive only the exact instances their invoked operations require.
3. Coverage resolves every derived instance to an authored public schema, an
   admitted receipt for an exact locally established subject, or a build-bound
   provider premise exported to the component manifest and discharged at
   composition. Anything else rejects.

Subject correspondence is exact: identity-preserving lineage, explicit
contract substitution, or an authored qualification-preserving transition.
The compiler never invents entailment from a value merely "descending from"
another subject. Static-machine binders are nominal, so a generic call obtains
premise schemas from the named requirement contract rather than from whichever
implementation is later substituted. This keeps generic and exported contract
identity fixed across selection.

The checked-only transition certificate covers an existing statement transfer
only when both endpoints are exact parameter- or prior-state-local-rooted
Field/Case places, possibly continued through nested in-bounds `FixedIndex`
segments of literal-length fixed arrays, and the copied fact carries
`CheckedTransformation` evidence. A
separate proof ledger retains both fact identities, the fact's source place,
the contextual source occurrence, destination place, formation point,
qualification payload/domain, and evidence. Checked progress independently
replays exact
structural place equality, construction order, statement-transfer origin and
point, formation ownership, unchanged payload/evidence, and the exact
member/attachment/local-declaration/array type walk with literal index bounds.
A local root must belong to the exact formation state and have exactly one
matching declaration strictly before the formation statement; producer and
replay independently re-resolve that declaration and its type. Label-only,
unknown, expression, type, runtime-indexed/ranged, nonliteral-length, generic,
and invalid-member relations produce no certificate. Each retained
structural place root must belong to the formation machine or exact formation
state; same-shaped foreign-machine or sibling-state parameter/local
substitution, and later, missing, duplicate, reordered, type-substituted, or
symbol-substituted local declarations reject independently for the
source fact, contextual source occurrence, and destination. The ledger changes
neither premise admission nor Terminal authority.

Durable provider-requirement artifacts retain those public schemas
structurally: profile identity, provider-receiver or caller-parameter subject,
the exact subject projections, and the profile owner's normalized closed
`established by` requirement-route set. A provider-receiver subject is recorded
as build-bound rather than rewritten as a caller parameter. The route set enters
provider-plan and component-manifest identity, but it is only an authorization
catalog: retention makes the dependency auditable and does not establish
membership. The canonical component manifest must still export each reachable
build-bound instance, and final composition must resolve it against the exact
selected provider occurrence and one non-forgeable admitted establishment
receipt issued through a retained authorized route.

The installation join is exact and transactional. One canonical registry
seals every selected provider plan to an installed provider occurrence before
it admits any profile receipt. A receipt separately names the qualified
subject occurrence and the issuer occurrence, the issuer plan, exact boundary
route, and one grant invocation. It is reusable proposition evidence for
several identical call-site demands, but its identity or invocation cannot be
rebound to another subject, profile, projection, issuer, or route. Component
closure retains the original pending manifest and exact receipt evidence; a
compact fingerprint or a mutable `discharged` flag is never authority.

Canonical terminal installation metadata commits both the manifest identity
and the opaque accepted-closure identity. Those numbers make substitution
visible in artifact identity but do not replace the retained closure: runnable
publication joins the canonical terminal object, image, installation record,
the linear `InstalledCode` claim itself, and opaque accepted closure.
Component-era publication retains that joined carrier until successful
retirement; rejected binding, publication, and retirement return the exact
custody unchanged. The installed-code claim therefore cannot be retired
independently while the runnable carrier is live. A compiler lane that emits a
native executable without those staged installation inputs stays fail closed
rather than treating selected provider plans as receipts.

The production deployment join preserves the complete selected provider-plan
closure independently of executions that happen to occur in one image.
Selected but unexecuted plans remain fingerprinted; executions must belong to
that set and exactly cover retained image settlements. The claimed registry is
carried through retryable provider/progress closure and remains inside the
runnable carrier until successful era retirement. Runnable decomposition is
unavailable before retirement. This owner currently accepts provider-
occurrence and progress records only; installed external-root handles borrow
installed code and require owned teardown before entering the same carrier.

The normalized guarantee records the actual pinned premises, not merely the
presence of `suspends` or `blocks` operational clauses. General machine-side
trace propositions, deadlines, starvation freedom, and entailment between
progress profiles remain deferred until a trace logic exists.

## Identity and revalidation firewall

Published contract identity contains the authored termination guarantee,
explicit premises, and terminal/failure contract. It excludes ranking
subjects, selected view, rank range, SCC mapping, and proof certificate.

Changing a provider from `Nat::Descending` to
`Nat::IncreasingTo(limit)` revalidates that provider and changes its proof
cache key. It does not change a requirement binding, a caller contract ID, or trigger
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
7. Every complete mutual call cycle decreases one joint ranking; forwarding
   edges cannot hide a preserving cycle.
8. `terminates` plus `suspends` remains conditional on the pinned wake/progress
   premises; the reach row alone cannot invent them.
9. An ungranted provider cannot self-assert a sealed progress profile; only an
   owner-classified domain's `established by` route plus its admitted receipt
   establishes one.
10. Checked dependencies instantiate from exact callee termination contracts;
    mention alone creates no premise, and every instance must be covered by an
    authored schema, exact receipt, or manifest-bound provider premise.
11. Swapping a provider's valid ranking witness revalidates that provider only;
    caller and requirement contract identities remain unchanged.

## Deferred, explicitly

- Source spelling for joint rankings across differently shaped mutual-cycle
  participants.
- General trace propositions and their proof calculus.
- Deadline, starvation-freedom, and quantitative progress contracts.
- Entailment or refinement between progress profiles after they cease to be
  opaque.
