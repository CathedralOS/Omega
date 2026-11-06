# Mathematical proof contracts

Omega's proof system uses ordinary machines, contracts, data, traits, and named
conformances. The [mathematical foundation](foundation.md) selects a dependent
core with proof-relevant identity and strict logical proofs. This specifies the
required model, not complete implementation support. The
[mathematical binding contract](mathematical_bindings.md) supplies the selected
source forms; [delivery obligations](#delivery-obligations) remain explicit.

## Machines and bundles

A machine's parameters bind subjects, `requires` supplies assumptions, and
`ensures` states conclusions its body must justify. Before importing a callee's
conclusions, a caller establishes the assumptions under the exact argument
substitution. Naming a condition does not prove it; failed proof search does
not prove its negation.

The same [call-precondition rule](../language/machines.md#call-preconditions)
also governs forming a mathematical result term, even if none of the callee's
conclusions is used. Proof-to-proof calls have no exemption. Named calls and
operator syntax retain the same selected contract in every context;
erasure, normalization, or knowledge that the underlying body
is total cannot remove a selected call's `requires`.

### Data identity in facts

Ratified 2026-11-06: explicit core identity replaces the contextual
`proof-fact-data-equality` rule. Value `left == right` selects the same equality
operation in executable expressions, `requires`, `ensures` and propositions.
Proof use must satisfy the selected invocation's ordinary totality, termination,
observation/effect and precondition rules. Type equality and specified primitive
relations, including cross-carrier integer comparisons, retain their rules.

`core::Identity<T>(left, right)` explicitly forms the foundation's proof-relevant
`Id T left right`. This is a sealed core mathematical declaration, not a new
keyword, operator, executable Boolean comparison or theorem asserting its own
conclusion. Its ordinary core declaration identity, typed subjects and universe
formation are retained; copying its spelling grants no privilege. It is usable
for proof-only and runtime-representable values without an `Equatable<T>`
algorithm. Forming the statement supplies no proof of it. A type with no selected
executable equality rejects `==` and the diagnostic points to explicit identity
when that is the intended claim. Identity itself does not require runtime code.

For example, a generic algebra's commutativity requirement is:

```omega
machine add_comm(a: T, b: T)
ensures core::Identity<T>(add(a, b), add(b, a));
```

This law states identity of results, independently of any selected comparison.
The body of each supplied theorem must prove it. The accepted source form and
compiler/core migration are tracked by **PROOF-KERNEL-CORE** in
[TASKS.md](../../../TASKS.md) and the [explicit-identity plan](../../drafts/designs/explicit_identity.md).

### Equality bridges and comparison facts

An executable equality fact does not itself license identity substitution.
Independent checked law evidence may establish either direction:

| Law premise | Conclusion | Use |
| --- | --- | --- |
| Selected `equal(a, b) == true` | `core::Identity<T>(a, b)` | Substitute identical subjects under the ordinary identity rules. |
| `core::Identity<T>(a, b)` | Selected `equal(a, b) == true` | Establish that comparison from identity. |

These are additional trait/conformance evidence, not optional members of
`Equatable`. Each selected conformance still supplies its complete row set.
A combined evidence trait may require both directions. Recognize the checked
law's form and exact applied equality row, not its name or another visible
implementation for the same `T`. Retain its premises, generic substitution,
subjects, value versions and assumption closure. A cited standalone theorem
may supply the same evidence; it is not automatically discovered by spelling.

Authorized core synthesis proves each direction only when the corresponding
component laws justify it. It must not grant substitution to IEEE float equality
or to a structural comparison containing such a component without further
evidence. For example, case-insensitive equality can reflect identity without
implying it. Same-carrier integer equality can support both directions;
cross-carrier numerical equality is not an ill-typed same-carrier `Id`.

Compiler-synthesized structural equality is proof-transparent through its
ordinary checked body: a true record comparison yields each selected field
comparison; sum comparisons retain tag agreement and the active payload case.
This does not turn IEEE field comparisons into identity or unfold invalid
payload projections. Synthesis must prove the constituent calls' admission and
requirements. User comparisons expose consequences through checked contracts,
cited evidence or otherwise admitted computation, not an assumed structural
interpretation based on their name or result type.

Repeated admitted pure comparisons with the same selected operation and exact
unchanged arguments have the same Boolean denotation. Thus `requires a == b`
can establish a body's unchanged `a == b` result as true without either identity
bridge. Mutations, observation-dependent inputs, different operation selections
or failed invocation admission invalidate that reasoning. This is call
congruence, not a general assertion that every comparison is reflexive.

A theorem-only machine has no Type result. Resultless law slots do not need dummy
returned witnesses. A Type-valued mathematical operation may instead denote a
value without an executable algorithm; it is not a resultless theorem or an
automatically executable call. The same checked contract may serve
runtime, compile-time, or proof use when its types and reach permit it. A fact-only
invocation emits no runtime call or work.

Traits bundle mathematical operations, witnesses, and checked laws. A named
conformance supplies the complete bundle. Its name, expected shape, or nearby
facts cannot supply a missing law or select an implementation. Preserve the
complete telescope, trait application, and normalized row map.

No dedicated formula declaration or authored logical-result machine is required
by this model. [Optional naming syntax](../../proposals/0001_proof_formula_syntax.md)
is an unproven ergonomic proposal, not a prerequisite for general logic.

## Mathematical values and quantification

Contracts must support universal and existential claims, nesting, and
quantification over arbitrary mathematical functions and predicates. Machine
parameters provide symbolic subjects, but enumeration of executable declaration
symbols cannot substitute for general mathematical quantification.

Use [machine-shaped logical hypotheses](mathematical_bindings.md#logical-hypotheses-and-machine-use)
for universal evidence and ordinary witness bundles plus squash for existence.
No quantifier keywords are introduced. Mathematical parameters range over terms;
closed top-level mathematical `let` definitions and prefix application construct
those terms without making ordinary machine calls partial. Logical evidence
cannot stand in for an operational implementation, even when it returns nothing
and reaches no service.

Proof-only values need not be executable. Constructive witnesses can be bundled;
nonconstructive existence need not produce a runtime witness. Choice and other
axiomatic reasoning retain their exact assumptions. Recursive mathematical data
without a finite runtime representation remain proof-only; this follows from
structure, not an extra proof-only type property.

Mathematical noncomputability is independent of carrier layout: a chosen member
of a nonempty subset of `u32` is a valid mathematical subject, but cannot be
materialized without a checked computation or realization. The
[foundation's separate judgments](foundation.md#separate-judgments) govern this
distinction; no new domain atom or runtime provider search stands in for it.

Runtime integer/address values enter mathematical integer arithmetic through the
total `embed` projection, retaining exact carrier range. Policy erasure with
`as` instead selects Exact carrier arithmetic and retains representability
obligations. Trapping arithmetic does not form a proof term. Floats project to
[FloatMeaning](../terminal-psi/mathematical_values.md#floatmeaning), preserving
signed zero, infinity, and NaN rather than pretending every float is rational.

`Real` is a proof-side abstraction over ordinary mathematical data and
equivalence/quotient machinery, not a runtime primitive or float mode. General
relation expressions and typed index telescopes precede evidence-bearing
quotients. Approximation theorems must explicitly connect Real specifications
to executable float implementations.

### Total arithmetic

Every term in a contract, domain predicate, or crash guard is total. Formation
is checked per operation, not per binding: comparison, equality, classification,
and total bitwise operations may consume Trapping-qualified values. An arithmetic
operation whose selected meaning can transfer control cannot form a proof term.
It is not silently read as Exact or mathematical arithmetic.

Exact operations discharge representability before formation. Wrapping and
Saturating discharge primitive definedness not resolved by their overflow policy,
such as a nonzero divisor. A term cannot use the condition containing that same
term to justify its own formation. Explicit policy erasure or `embed` selects
the intended total reading; the two are not equivalent.

Integer/address embedding is injective within its exact source carrier and
preserves that carrier's range. It neither mutates the source nor creates runtime
storage. Computed operands preserve their selected policy: embedding a wrapped
sum does not turn it into unbounded addition. A call interpreted denotationally
needs a checked observation-free, reach-free, crash-free, terminating invocation,
not merely a same-spelled mathematical operation.

Builtin fixed-integer equality and order have the same meaning as the relation
between the operands' exact `embed` projections, including operands with different
widths or signedness. This characterizes the comparison; authors need no explicit
embedding or cast to compare admitted runtime integer values. Each projection
retains its source carrier and subject, and equal values do not identify their
types. Proof and executable comparisons obey the same
[integer relation](../language/numeric_values.md#integer-equality-and-order).

`embed` cannot be used as an executable conversion. Boolean and noninteger
carriers have no integer embedding merely because their operands are integers;
float meaning uses its separate projection.

All integer/address carriers embed into signed proof `Int`, so subtraction is
ordinary signed subtraction even for unsigned source values. Exact conversion
to `Nat` requires nonnegativity. Ordinary `Nat - Nat` requires right <= left;
explicit `Nat::saturating_sub` instead denotes `max(left - right, 0)`.
`Nat` includes zero: forming exact `n - 1` needs `1 <= n`, not `1 < n`.
The two operands being natural numbers alone does not establish their order.
Named `Nat::subtract(left, right)` has the same order premise as `left - right`;
the total saturating operation remains a distinct contract. This choice of
subtraction meaning is identical in proof and executable uses wherever admitted.
Target-relative bounds retain the exact observation dependency. An exclusive
one-past address bound may be representable in proof mathematics without fitting
the runtime address carrier.

For mathematical primitive result `M` and result-carrier bounds `[MIN, MAX]`,
the selected catalog supplies these denotations after primitive definedness:

| Policy | Law |
| --- | --- |
| Exact | Formation proves `MIN <= M <= MAX`; embedded result equals `M`. |
| Wrapping | Embedded result equals width-specific wrapping of `M`. |
| Saturating | Embedded result equals `M` clamped to the carrier. |
| Trapping | A normal return embeds to `M`; the exact primitive trap predicate governs failure. |

The trap predicate is not a generic range test: division includes zero and
signed minimum divided by -1, while shift counts and floats follow their own
[numeric rules](../language/numeric_values.md). The compiler defines the
primitive's meaning; an authored crash guard only bounds its possible failures.
[Crash coverage](../language/effects.md#guarded-crashes) checks that bound.
`ensures` constrains normal returns, not crash paths.

## Citation and induction

An ordinary statement call cites a checked theorem. Establish its `requires`
under exact operand substitution before importing its `ensures`; erase the call
if fact-only. Imports do not activate global rewrite rules. Diagnostics may
suggest lemmas, but the citation belongs
in source.

A recursive citation may use the recursive contract only after the caller
establishes its premises for the recursive arguments and its exact edge proves
strict decrease under the component's well-founded ranking. Descent licenses
induction, not arbitrary preconditions; there is no recursive-component exemption
from premise checking. The citation's own conclusions cannot establish its
premises or license its own descent. This applies to resultless, discarded, and
nested calls whenever their conclusions are used.
Mutual recursion requires a joint ranking; citation cycles cannot justify
themselves.

Proof/compile-time recursion may rank on mathematical structure. Runtime
recursion is tail-only and lowers to constant-stack iteration; non-tail runtime
recursion rejects. Transition backedges are jumps and may remain productive
indefinitely. A guarded unsigned predecessor can connect arithmetic descent
with structural induction, but each checker must establish its own part of
that bridge rather than invent facts in the other domain.

Measure decrease, accessibility evidence, a mathematical denotation and loop
lowering answer different questions. Check termination, denotation correctness
and generated-loop correspondence independently. Runtime iteration does not
decide the logical relevance or elimination rules of accessibility evidence.

## Identity, availability, and erasure

Keep statements, witness values, and derivation provenance distinct. Logical
expressions retain exact binders, subjects, substitutions, and dependencies;
display names are not truth or reusable proof identity. Distinct Type witnesses
do not become one value merely by proving the same statement. Strict logical
proofs of the same complete application are definitionally irrelevant under
the [foundation](foundation.md#logical-proofs-and-mathematical-witnesses), without
identifying their provenance. Forwarding and repeated projection preserve the
selected Type witness identity.

Guarantees are path- and result-sensitive. Every ordinary exit establishes its
applicable conclusions; caller import requires the matching result case and
argument/result substitution. Validity is the intersection of referenced
occurrence and bundle scopes. Intersecting writes invalidate borrowed or
revisioned facts. A bundle cannot make a conditional fact unconditional.

Public trait requirements and selected private realizations retain separate
contracts. Replay rejoins requirement, owner-scoped application, realization,
and ordinary call. Private strengthening does not become a public guarantee or
select a different executable lowering family.

Relation applications retain independently bound left/right carrier-index packs.
Constructor lifts, dependency-ordered field relations, and required transport
proofs remain in the rows justifying the operation. Callable telescopes use
positional identity; parameter spellings are diagnostic metadata.

Erased logical facts carry no consumable authority or usage-count obligation.
One-shot permissions remain affine or linear Type carriers even without runtime
layout. Erased Type witnesses retain identity, multiplicity, validity,
conservation obligations, and provenance. Neither erased kind acquires an
executable storage place or cleanup action merely by supplying proof evidence.

Structural field rows retain authored relevance and exact normalized type
identity. Erased fields need not enter the executable structural-type graph;
layout skips them before ABI classification. Semantic fingerprints retain them.
Mismatched relevance/type rows reject.

### Explicit erased bindings

`[erased]` marks a binding occurrence, independently of the carrier's
multiplicity and validity:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The binding remains in checking and semantic identity but contributes no runtime
field, address, read, or cleanup. It may supply proof computation or static
authorization; it cannot determine runtime data or control. A zero-layout Type
value is not implicitly erased. Construction supplies the erased term unless a
visible accessible nullary constructor determines it structurally; there is no
general implicit inhabitance or default-value search.

Evidence attached to executable work binds its exact operation, callee, result,
and specialization, not a second inferred invocation. Independent replay rejects
orphan witnesses, missing laws, changed substitutions, stale scopes, and wrong
runtime links. Erasure does not erase transitive assumptions.

## Licensed normalization

Algebraic normalization requires an explicitly selected conformance with checked
operation and law slots. One closed conformance supplies all inherited slots
through checked members, explicit machine references, or its default
instantiations. A standalone exact-requirement satisfier is not a selectable
algebra, and similarly named lemmas do not license normalization.

Computed index expressions use this ordinary trait mechanism. A consumer that
needs to reorder operands requires a trait containing the operation and its
commutativity law; the caller supplies a checked conformance through the existing
explicit conformance binding. Reassociation additionally requires associativity.
Resolve the operation and its law evidence from that same selected map and retain
their identities through substitution. Neither a token such as `+` nor a unique
visible satisfier supplies the evidence. No domain-specific algebra-selection
clause, per-expression binder, or index-operation registry is introduced.

Normalize only operations licensed by that instance. Associativity,
commutativity, distributivity, and zero/one identities require their respective
laws; one does not imply another. Literal-linked identities follow the checked
connection rule below. Search limits and unequal normal forms do not establish
inequality.

Having a static index does not itself require commutativity or associativity.
Those laws are required only for transformations that consume them. Without a
law, retain the expression and use ordinary computation or established exact
facts where available; do not reject noncommutative index operations merely
because a commutative normalization would have been convenient.

Closed proof-static indexes are canonical values. Open indexes normalize only
under the selected algebra and its checked public operation contract. Local
facts establish remaining compatibility without changing index identity.
Identity-bearing algebra evidence must be derived, not admitted. Retain the
algebra instance, normalized operation-contract identity, canonical expression,
compatibility evidence, and normalizer version. That version is provenance;
canonical policy changes must be recorded, not silently interpreted as the same
policy. Internal provenance changes are not a language-release prerequisite.

Index identity retains the exact selected provider and applied conformance
operands, including operations with neither AC law. Aliases remain
diagnostic names. Closed integer leaves share canonical value identity across
expression and named-const forms. Established equality operands use this same
normalizer under simultaneous call bindings; they do not change declared type
identities or license an operation from a different conformance application.

### Literal-linked index identities

Named decision: `literal-linked-index-identities`, settled 2026-11-03.

A generic program can identify `Indexed<N + 0>` with `Indexed<N>` when its
selected algebra proves that adding the literal zero preserves the operand.
This changes canonical application identity in signatures, conformance
arguments, specialization keys, and value transfers alike; it is not merely a
compatibility conversion at a value boundary.

An abstract identity law such as `core::Identity<T>(add(value, zero()), value)`
combines with a checked connection law `core::Identity<T>(zero(), 0)` with the
literal formed in the exact carrier `T`. Multiplication similarly combines
`core::Identity<T>(mul(value, one()), value)` with
`core::Identity<T>(one(), 1)`. An ordinary index-specific trait
may add those connections while inheriting general algebra operations and
laws. A consumer requests that contract through its explicit conformance
binder, and callers supply the implementation. No new syntax, implicit algebra,
or proof citation at each use is introduced. Each rewrite needs only its own
laws, not a complete ring or semiring.

Collect the complete selected conformance's direct and inherited law slots under
their exact applications and substitutions. Recognize law structure and
mathematical equality, never requirement names or operator punctuation; match
operations and nullary slots by exact requirement identity, not same-spelled
names. Both laws must refer to the same selected conformance application and
its corresponding slots. The connection names the exact typed integer literal.
Identity-bearing laws must be checked, unconditional guarantees; a law with an
undischarged premise cannot license a canonical rewrite. Concrete providers may
use ordinary admitted computation or citation to prove a connection law, but
generic use consumes the published law without evaluating an abstract machine
or unfolding its implementation inside type equality.

Canonical integer indices may retain domain qualifications, including an
arithmetic policy. The conformance bound selects the operation; qualified
operand types constrain matching and do not supply a default implementation.
Retain the exact carrier, qualifications, selected operation, and law evidence.
Wrapping, Saturating, and Exact do not grant any algebraic law by their names.
For example, signed saturating addition is not associative: in `i8`,
`(100 + 100) + -100` gives `27`, but `100 + (100 + -100)` gives `100`.

Left and right identities are separate laws. A right identity removes a right
operand, or an identity with a preceding operand in an associative chain;
removing a leading identity needs a left-identity or commutativity law. A chain
of identity elements reduces to the correctly typed identity, not an empty
expression. `N + 1` is not an identity. `A * (B + 1)` may compose distributivity
and multiplicative identity into `A * B + A`; removing `A * 0` separately needs
an annihilation law and its literal connection.

Discharge the authored expression's formation obligations independently of
canonical identity simplification, and justify every newly introduced
operation. Normalization may help prove an obligation but cannot remove it or
use the goal to justify its own formation. Thus `0 * (B + C)` cannot erase the
Exact representability obligation on `B + C`, and regrouping cannot hide an
overflowing intermediate. An operation with preconditions is not categorically
excluded: Exact `N + 0` is formed for every `u8 N`, and its checked identity law
remains usable. Full unconditional algebra laws still owe their full formation
and totality contracts.

Remove licensed identities before considering expansion and normalize generated
terms again, under the complete-chain and budget rules below. Normalizer
versions record policy provenance, not a new nominal type; unreleased
development artifacts may be rebuilt rather than requiring historical-normalizer
support or an artifact migration framework.

### Bounded canonical index normalization

Named decision: `bounded-canonical-index-normalization`, settled 2026-11-03.

A generic library can use a value indexed by `A * (B + C)` where
`A * B + A * C` is required when the exact selected algebra licenses that
distributivity. Normalize toward expanded sums of products, not general
factoring. This is canonical index identity in signatures, conformance
arguments, specialization keys, and value transfers, not an instruction to
expand runtime arithmetic or every compiler expression tree.

Resolve operand types and qualifications and bind exact operation contracts
before applying algebraic rules. A generic conformance binder supplies its
abstract requirement identities and checked public laws without a concrete
provider body or known index value. Neither physical layout, ABI placement,
native target realization, nor monomorphization is a prerequisite. Formation
remains an independent obligation; normalization cannot justify its own partial
operand or remove an authored overflow requirement.

The canonical identity profile specifies a target-neutral encoding of typed
leaves and selected operation applications. Retain the exact carrier and
qualification identities, applied algebra and requirement identities, and
ordered child encodings. Flatten a chain only under its own associativity law;
sort operands by canonical encoding only under commutativity. Preserve factor
order and grouping when their laws are absent, and preserve repeated operands.
All producers of a published identity use the same schema and ordering, not
compiler-private arena handles, diagnostic names, debug formatting, or hash-map
iteration order. In-memory trees, shared nodes, or compact polynomial storage
remain implementation choices and must export the same identity encoding.

Internal coefficients may encode repetition of the same term, not the meaning
of an authored integer literal. The zero/one connections above do not establish
`N + N == 2 * N` or `N + 1 + 1 == N + 2`. A selected abstract operation on
literal operands is not builtin integer arithmetic. Closed proof-static indices
instead use ordinary admitted evaluation after their complete arguments and
concrete implementation are supplied; their keys are canonical values. This
evaluation owes the usual formation, admission, and resource obligations, and
must agree with the checked generic evidence.

Before distributive expansion, collect the whole same-authority associative
operation chain. Do not expand an inner product before it can join that chain.
Normalize its non-chain operands, remove licensed identities, and apply licensed
ordering to obtain the canonical pre-expansion form. Decide expansion from
that form, never the authored parenthesization. After an admitted expansion,
normalize the generated products and sums under their own selected laws. In
particular, associativity must keep `(A * (B + C)) * D` and
`A * ((B + C) * D)` coherent when multiplication is not commutative.

The expansion limit is 64 resulting summands per canonical product expansion.
Count the prospective summands licensed expansion would produce before
allocating them. With commutative multiplication, the measure is the product
of the available sum-factor alternative counts; without it, left
distributivity can expand only the rightmost sum and the measure is that sum's
alternative count. The measure uses simplified operands and preserves any
grouping for which no associativity law exists. Use bounded counting rather
than materializing an oversized expansion to discover its size.

Past the limit, retain that product's canonical pre-expansion form with its
normalized non-chain operands; do not retain a parenthesization-dependent
partially expanded chain. Retain cutoff evidence naming the product, measure,
and limit. This does not reject a well-formed type merely because it appears
in a signature or key, and identical retained forms may still compare equal.
If compatibility cannot otherwise be established and a cutoff obstructed the
comparison, diagnose "equality not established within the normalization budget"
with that measure and limit, not mathematical inequality. Ordinary exact local
facts or checked theorem guarantees can establish the remaining compatibility;
using distributivity directly with a large operand need not expand that operand.
No localized canonical-budget override syntax or implicit theorem search is
introduced. Additional proof effort cannot change the canonical identity policy.

For a fixed selected law set and canonical pre-expansion form, normalization is
deterministic and idempotent and produces the same identity and budget decision.
Below the larger law set's expansion limit, adding valid laws preserves
identifications from the smaller set, subject to the mutual-distributivity
exception below. Cutoffs can prevent that stronger-law identification; the
compatibility diagnostic must identify the budget rather than silently promise
unconditional monotonicity. Multiple algebras can coexist, but their identities
and evidence do not mix. Changing a trait's laws changes its published contract.

Named exception: `mutual-distributivity-no-expansion`. If the selected algebra
declares that each of two operations distributes over the other, expand neither
direction. Do not choose a direction from punctuation or add a new lattice
normalizer without a concrete customer. This exception can change an
identification even below the limit and is excluded from the monotonicity claim.

Within one compilation, carry checked generic compatibility derivations through
the exact specialization substitutions rather than rediscovering the same
equality with another expansion attempt. This is evidence transport, not an
unchecked success flag; call preconditions, formation, and target obligations
still apply. Verification of foreign or untrusted evidence remains at its trust
boundary. The canonical schema, rule set, measure, limit, fallback, and exception
belong to one recorded policy; root proof-work sponsorship and its resource
receipts do not alter that policy's index identities.

## Axioms and receiving policy

A theorem's meaning includes its calculus and exact transitive assumptions.
Axioms are selectable, not compulsory library truths. A dependency cannot accept
its assumptions on behalf of a consumer. An inconsistent assumption set can
prove false statements; derivation checking does not certify consistency.
Mathematical admission does not silently grant runtime-safety or artifact
acceptance authority.

Compute assumption closure over the complete reachable checked declaration
graph, including statements, types, bodies/evidence and referenced declarations.
It is independent of whether conversion unfolds a definition. Normalization,
irrelevance and erasure cannot silently shrink this record. Unused declarations
and discarded search attempts are not dependencies merely by sharing a package.
Mathematical axioms are explicitly classified in checked evidence; result-bearing
axioms are not ordinary missing executable-provider slots.

Unproved facts enter through admission-bearing boundary contracts and root
grants, not scattered unchecked assumptions. A bodyless boundary machine's
`ensures` is an axiom claim until admitted, not a checked theorem. A receipt
binds human policy and a domain-separated digest of the exact selected provider
plan, generic machine template, or nongeneric machine contract. Compact report
fingerprints cannot settle admission.

A generic axiom over a machine binder spends its grant on the normalized
template and required machine contract. Instances retain their selected contract
identity without spending another grant. Instance-specific trust uses a
nongeneric admitted fact.

A deferral waives one generated obligation, creates no reusable fact, is pinned
to its site, warns each build, and cannot cross package release. A permanent
claim needs a reviewed boundary contract; packages cannot self-grant it.

Own-package boundary claims may be used during development with a standing
warning until granted. Imported claims remain inert until the consumer grants
them. Development use is not consumer or release acceptance. A provider-slot
grant covers only its exact selected plan; unselected or partial candidates
cannot inherit that receipt merely by implementing the same requirement.

A boundary claim that the checker can refute against established bounds, domains,
or accepted statements rejects even when ordinarily granted. The sole exception
here is the exact semantic relationship of an explicitly accepted root-assumed
runtime binding, described below. This veto is not a proof that the complete
assumption set is consistent.

Root [assumed binding contracts](../build/assumed_binding_contracts.md) record
an unproved relationship between two runtime boundary contracts, not a checked
`satisfies` edge or an exemption for either body. Their exact premises survive
through the normal assumption closure, including statements and erased evidence.
They are inadmissible for build-time computation. An explicitly accepted
relationship may override the refutable-claim veto at that exact crossing,
including when the supplied guarantee contradicts the caller-facing guarantee.
Both statements and the refutation remain in its trust record under
[known contradictions](../build/assumed_binding_contracts.md#known-contradictions-at-the-trust-crossing).
Neither body nor any ordinary conformance gains an exemption from its own checks.
The assumption is not a checked proof of the relationship; derived guarantees
remain conditional on it. Registration grants no ambient proposition, permits
no malformed derivation, and cannot make its assumptions admissible for
build-time evaluation. Ordinary mathematical axioms retain the veto above.

### Admission and runtime diagnostics

An admitted claim is trusted without mandatory runtime checking. Runtime
checkability is not a condition of admission, and enabling or disabling
diagnostics does not change grants or the reported assumption closure. Omega
makes no commitment to automatic assertion insertion, runtime proof execution,
or a special proof-build mode. Authors are not required to replace automatic
instrumentation with manually written checks.

Authors may test providers or write ordinary executable validation. A test can
expose a violation on an observed execution; passing tests establish neither
admission nor universal correctness. A meaningful test of a claim cannot rely
on that claim, or conclusions derived from it, to establish its result. A
comparison after a call may already follow from the call's admitted guarantees
and can be eliminated under ordinary optimization.

To validate rather than trust a particular guarantee, expose the result through
a boundary contract that does not assume it, then use checked executable
validation to establish the property on the successful path. Calling, storage,
lifetime, and observation safety still require their own justified premises;
removing one disputed guarantee does not make all foreign execution untrusted
or safe by default. Validation establishes facts about its exact observed
subjects, not a universal provider promise. A post-call check cannot undo
external effects or contain arbitrary provider corruption.

[Requirement-based tests](../build/testing.md) obey the same rule. Their
registration changes execution roots, not the facts available to checking or
optimization. Passing a test grants no admission and is not a universal proof;
mandatory verdict checks remain distinct from optional diagnostic assertions.

[Build-level behavior exclusions](../build/behavior_exclusions.md) let a selected
composition establish absence of named crashes, services or physical authority
classes despite conservative public allowances. Assertions remain ordinary
checked code with eager arguments; no new assertion primitive or crash cause is
introduced. A checking/no-op requirement cannot promise that normal return proves
the condition. Exclusion evidence neither validates admitted assumptions nor
introduces mandatory runtime checks, and does not narrow ordinary API contracts.

Checked proof evidence retains theorem identity, cited lemmas and premises,
normalization licenses, derivation/checker version, and trust/deferral closure.
Deterministic normalization owns identity; stronger proof search may establish
more claims but cannot rename them. Terminal obligations follow the
[fact-validity rules](../terminal-psi/fact_validity.md).

## Certified elaboration and review

Source presents a proof strategy, not every primitive inference. Local
computation, constructors, branch facts, contract extraction, and licensed
decision procedures may be implicit, but acceptance needs checked evidence under
exact premises. Theorem, conformance, boundary, and other provenance-bearing
dependencies remain explicit in source even when resolution is unique.

A specified total procedure may be replayed by the checker. Partial or heuristic
search must instead supply evidence checked without repeating the search. A
replayed normalizer remains trusted checker logic unless it emits a lower-level
certificate; totality alone does not establish soundness. Normalization cites
its exact conformance and laws and inherits their complete assumption closure.

Derive the review synopsis deterministically from the checked certificate and
its source-attribution metadata, never a second analysis of what probably
happened. It reports certificate identity, recursive components, implicit closure
rules, cited laws, and trust closure. The synopsis is explanatory; the complete
certificate and its checked question remain authoritative.

[Quotients](quotients.md) owns representative independence, selected theorem
roles, constructor relations, executable observers, and published correspondence.

## Delivery obligations

The [foundation](foundation.md) is the owner of the selected mathematical model,
not the current Rust vocabulary or a collection of proof-assistant defaults.
The [mathematical source forms](mathematical_bindings.md) are selected. Binding,
substitution, universe inference, checked logical evidence and witness/assumption
integration remain implementation work, not an unresolved notation choice.

Changing axioms within one calculus does not establish compatibility between
different calculi. These source forms preserve the specified evaluation,
erasure, assumption-tracking and verifier-obligation rules. They authorize neither
an arbitrary checker-plugin mechanism nor retired formula-naming syntax.

Certificate byte layouts, a tactic-machine API, and the remaining
Real/approximation library are separate engineering or deferred surface work;
they are not substitutes for implementing the selected foundation. Promote any
new semantic or trust choice they expose to the owner queue before relying on it.

The [W-based inductive profile](inductive_profile.md) and
[set-quotient interface](quotients.md#set-quotient-foundation) are selected.
Combined metatheory, encoding correctness, kernel checking and library proofs
remain execution obligations, not completed results or pending architecture votes.

`PROOF-CONTRACT-MIGRATION` on the [execution board](../../../TASKS.md) owns worked
proofs and migration through source, serialization, and replay. Neither a
mechanical rewrite nor a few successful proofs establishes mathematical
completeness. Current automation belongs beside [validation](../../../omega-rust/psi/compilation/validation/README.md).
