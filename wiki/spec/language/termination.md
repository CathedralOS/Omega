# Termination, ranking, and progress

Termination is a positive eventual-terminal-outcome guarantee under authored
requirements and pinned callee/provider progress premises. It does not promise
a particular result, fairness, wakeup, deadlines, no suspension, or no blocking.
`ensures` is partial correctness: it constrains a reached return edge but does
not prove that edge is reached. [Effects](effects.md) independently constrains
possible suspension, blocking, and crashes.

Permitted endpoints include normal return, a covered crash, and the canonical
[process-exit transfer](process_exit.md). The promise authorizes none of them;
crash contracts and service reach/capability requirements remain independent.
A machine that conditionally exits and otherwise returns can satisfy
`terminates` when every admitted path has the required progress. An exit leaf
needs no further cycle decrease, but its operands and preceding calls still
require progress. Excluding crashes and all external-terminal services together
with termination establishes normal return under the stated premises.

An exit does not establish a normal-return `ensures` clause and does not
necessarily violate it. Unconditional eventual-disposition guarantees must
account for terminal outcomes separately; ranking alone does not prove release.

## Published guarantees and private witnesses

The source clause family separates a public promise from implementation evidence:

```omega
terminates;
terminates by remaining;
terminates by items -> Slice::Length;
terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
```

Bare `terminates;` authors the promise. Omission on a bodyless requirement or
export publishes no guarantee, even when the current checked body is acyclic.
An implementation satisfying a requirement inherits its guarantee and premises;
it need not repeat the promise. A witness discharges that promise rather than
redefining it.

A private acyclic body needs no annotation and derives a local `Terminates`
summary. Local direct calls may use that exact checked summary; trait, generic,
and exported calls use authored or inherited promises. A cyclic terminating
body must supply its witness; inference never chooses its subjects or silently
publishes a guarantee. Unknown progress dependencies prevent a checked guarantee.

Published identity retains the guarantee, explicit premises, and terminal/failure
contract. Ranking subjects, view, rank range, cyclic-component mapping, and proof
certificate remain private. Changing a valid witness revalidates the provider
and its proof cache, not caller/requirement identity or contract-driven dependent
compilation. Explicit complexity/resource promises belong to their own public
contracts, not hidden ranking data.

Declared measures are package-private; `pub measure` rejects, and other packages
cannot name them directly. An exported machine may use its package's private
measure because callers depend on the promise, not the witness. Cross-package
operational recursion remains unsupported pending a compositional termination
interface that closes from published evidence without inspecting private measures.

## Ranking

`terminates by subject -> View` selects a well-founded ranking theory. The view
may be descending naturals, an increasing value approaching a fixed bound,
proper subtree, or a lexicographic product. An unbounded increasing view is not
well-founded. Authors need not synthesize subtraction to express a bounded
increasing cursor; the view owns normalization.

An optional `in lower..=upper` constrains the rank produced by the view, not
storage capacity. Its lower bound establishes the floor. Short form without
`-> View` requires a carrier-declared stable canonical default, immediately
elaborated to an explicit view. A uniquely visible user measure is not a default;
adding another declaration cannot reinterpret existing code.

Terminating state cycles require strict descent on every cyclic transition.
Mutually recursive calls instead use one joint ranking for the entire strongly
connected call component: edges may preserve rank only if every complete cycle
contains strict descent. A DFS-selected subset of cycles is insufficient.
The exact source spelling for differently shaped mutual participants remains
undetermined; the joint-cycle semantic rule is settled.

One component cites its relation and well-foundedness evidence once, with a
comparison proof for every internal edge and a global every-cycle descent check.
Local decreases cannot prove well-foundedness. An admitted custom ranking theory
makes dependent recursive proofs admission-dependent. Calls outside the component
use ordinary contract application. [Terminal control](../terminal-psi/control_flow.md)
defines reconstruction and grouped certificates.

This certificate establishes eventual termination. It does not authorize
importing a recursive callee's contract as an induction hypothesis on a
rank-preserving edge. That import separately requires strict descent at the exact
edge under [proof citation](../proofs/contracts.md#citation-and-induction),
regardless of call syntax or runtime erasure.

Computed next subjects use ordinary decrease obligations and cited lemmas; strict
syntactic subterms are an automation tier, not the semantic limit. Rank ranges,
argumented-view bounds, subject substitutions, and all comparisons retain exact
normalized witness identity. Checked diagnostics use `terminates by`, not a
retired alternative spelling.

## Calls and productive loops

Runtime recursive call cycles are tail-position only and lower to constant-stack
iteration. Ranking proves termination; tail position permits that lowering.
Ranked non-tail recursion is valid in proof/compile-time use and rejects runtime
lowering. After recursive cycles lower to backedges, the runtime call graph is
acyclic for activation-storage planning. This is context-sensitive eligibility
of one machine construct, not a separate proof language.

A transition backedge remains within its activation. A productive transition
loop without a termination promise may run forever and owes no rank; productivity
alone proves no fairness or eventual response. When termination of an algorithm
cannot be proved, a bounded API takes explicit fuel and returns exhaustion as an
ordinary case. That algorithm-visible budget is distinct from
[logical-work accounting](../resources/logical_work.md) and creates no native
runtime meter.

Logical obligations still reject unless proved. An operation whose selected
semantics explicitly permits a crash must fit the enclosing crash contract;
declaring that ceiling does not make an unproved Exact operation trapping.
Recoverable errors and cooperative cancellation
remain result cases. A crash is not normal return; its frontier is an audit
lower bound, not proof that unlisted state remains usable. Termination never
names a crash cause or substitutes for the independent cleanup/control contract.

## Ranked callees on projected receivers

An ordinary projected call such as `compiler.parser.scan(...)` borrows the
parser field as the callee's whole receiver. The same original referent remains
`self` through ranked backedges and return. The callee needs neither the enclosing
nominal type nor a second parent reference. Caller evidence establishes exact
projection, type, access, lifetime, and parent relationship; conflicting parent
access rejects while the subloan is live. Explicitly retaining both parent and
child references requires their represented loan relationship, not an exception.

A transition does not rebind `self` to another object. Linked traversal may carry
a changing node reference as an ordinary cursor parameter, with a remaining
length, budget, or structural measure. These choices do not introduce new
receiver syntax or imply general loop-carried-reference implementation support.

The caller/callee join independently proves argument/reference preparation,
callee ranking, return/cleanup, and composed resources. An ordinary outer call
may have a frame; iterations add no frames. A standalone ranked entry or widened
parameter count does not prove composed call support. Preserve exact projection,
callee identity, referent, and caller-visible writes through verification and
native replay. The [native implementation note](../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#ranked-native-admission)
tracks that bounded entrance.

## Opaque progress profiles

A progress profile is a named opaque semantic domain over a boundary-provider
capability, explicitly classified by its owner with `satisfies ProgressProfile`
and exact `established by` routes. Classification cannot be inferred from an
empty predicate, provider route, or terminating use. One atomic domain has at
most one such classification; downstream packages cannot append it or its routes.

Profiles are routed and predicate-free: they supply no predicates/operators,
cannot flow-narrow into existence, do not enter the ordinary proof-fact catalog,
and entail no other profile. Only the profile owner or explicit acceptance
authority may authorize admitted establishment. Imported claims are inert until
granted; packages cannot self-grant, and receipts/reporting expose the expenditure.
Profiles participate in deterministic provider/slot admission.

An ordinary `requires scheduler in WeakFair` attaches the profile to that exact
subject. There is no separate machine clause or ambient progress promise.
Premise attachment distinguishes:

| Level | Obligation |
| --- | --- |
| Public schema | A bodyless requirement/export authors its profile-qualified subjects; an exported implementation covers every body-derived dependency without rewriting that public set. |
| Checked dependency | Instantiate the exact selected operation's termination premises under its actual argument substitution. Receiving, mentioning, suspending with, or forwarding a qualified value alone adds none. |
| Coverage | Resolve each derived instance to an authored public schema, an admitted receipt for the exact local subject, or a build-bound provider premise retained in the manifest and discharged at composition. |

Static-machine binders use their nominal requirement's premises, not those of a
later replacement implementation. Private mutual components propagate finite
sets of exact subject-qualified premises and build-bound demands. Growing
projected demands are neither generalized into universal qualification nor
discarded. Forwarding an unused recursive reference creates no dependency.

## Subject preservation

Progress evidence follows exact identity-preserving lineage, explicit contract
substitution, or checked qualification-preserving transitions. A value merely
descending from another value is insufficient. [State contracts](state_contracts.md)
governs capture and invalidation; these additional constraints apply to premises:

- Owned replacement captures the source value at assignment, not later source
  contents. Disjoint writes preserve correspondence; an unchanged field name
  does not preserve the replaced value's qualification.
- Reference copies freeze bindings, not referents. Rebinding one alias leaves
  earlier copies unchanged. Unknown references and their copies remain unknown;
  later recovery cannot retroactively identify them.
- Reference leaves in records/cases require exact declared selectors and load
  evidence while the carrier's reference-binding prefix remains frozen.
  Replacing or exposing that prefix through any known alias invalidates the
  relation. Each additional reference boundary needs independent load evidence;
  an empty write frame or lifetime elision cannot supply it.
- Direct reference returns and references inside owned results retain the same
  checked source/body relationship. A nominal result type or qualification
  cannot replace the helper's binding history. The helper's writes remain an
  independent check against preserving the referent's prior qualification.
- Operand exposure is checked independently of the callee write frame. An
  unrelated unknown reference does not erase established correspondence, but
  unknown writes and exposed bindings cannot prove preservation.

State arrivals retain independent field origins and every finite entry-subject
alternative. A finite permutation is not ambiguity or unbounded premise growth;
repeated projection without a finite closure cannot preserve the original
qualification. Named-state and entry backedges use exact target-state formals,
not the machine's still-forming summary as if the jump were a nested call.
Reference identity may transport an existing live qualification, never establish
a missing or invalidated one. Bounded source-certificate support remains beside
[checked progress](../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md#flow-ranges-and-progress).

## Composition and limits

Provider artifacts retain public profile schemas structurally: profile identity,
caller parameter or provider-receiver subject, exact projections, and owner route
set. Provider receivers remain build-bound rather than relabeled caller inputs.
Routes are authorization catalogs, not membership. Composition resolves every
reachable build-bound instance to its exact selected occurrence and admitted
route receipt. A receipt may serve identical call demands but cannot rebind
subject, projection, profile, issuer, or grant invocation. Issuer and subject
may be different occurrences. [Component publication](../build/component_publication.md#installed-provider-and-progress-closure)
owns transactional installation and retained runnable custody.

Termination is not WCET. Fixed logical work additionally bounds transitive calls
under exact provider summaries; a rank range alone is insufficient. Target timing,
memory latency, suspension, deadlines, starvation freedom, general trace logic,
and entailment between currently opaque profiles require separate specified
analysis. They are not inferred guarantees of `terminates`.
