# Mathematical proof contracts

Omega's proof system uses ordinary machines, contracts, data, traits, and named
conformances. This specifies the required model, not complete implementation
support. General source binders and foundational rules still need specification
as listed under [undetermined foundations](#undetermined-foundations).

## Machines and bundles

A machine's parameters bind subjects, `requires` supplies assumptions, and
`ensures` states conclusions its body must justify. Before importing a callee's
conclusions, a caller establishes the assumptions under the exact argument
substitution. Naming a condition does not prove it; failed proof search does
not prove its negation.

A theorem-only machine has no Type result. A result belongs to a machine that
computes an observed value as well as establishing its contract. Resultless law
slots do not need dummy returned witnesses. The same checked contract may serve
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

Proof-only values need not be executable. Constructive witnesses can be bundled;
nonconstructive existence need not produce a runtime witness. Choice and other
axiomatic reasoning retain their exact assumptions. Recursive mathematical data
without a finite runtime representation remain proof-only; this follows from
structure, not an extra proof-only type property.

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

`embed` cannot be used as an executable conversion. Boolean and noninteger
carriers have no integer embedding merely because their operands are integers;
float meaning uses its separate projection.

All integer/address carriers embed into signed proof `Int`, so subtraction is
ordinary signed subtraction even for unsigned source values. Exact conversion
to `Nat` requires nonnegativity. Ordinary `Nat - Nat` requires right <= left;
explicit `Nat::saturating_sub` instead denotes `max(left - right, 0)`.
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

An ordinary statement call cites a checked theorem. Import its `ensures` under
exact operand substitution; erase the call if fact-only. Imports do not activate
global rewrite rules. Diagnostics may suggest lemmas, but the citation belongs
in source.

A recursive citation may use the recursive contract only after its exact edge
proves strict decrease under the component's well-founded ranking. This applies
to resultless, discarded, and nested calls whenever their conclusions are used.
Mutual recursion requires a joint ranking; citation cycles cannot justify
themselves.

Proof/compile-time recursion may rank on mathematical structure. Runtime
recursion is tail-only and lowers to constant-stack iteration; non-tail runtime
recursion rejects. Transition backedges are jumps and may remain productive
indefinitely. A guarded unsigned predecessor can connect arithmetic descent
with structural induction, but each checker must establish its own part of
that bridge rather than invent facts in the other domain.

## Identity, availability, and erasure

Keep statements, witness values, and derivation provenance distinct. Logical
expressions retain exact binders, subjects, substitutions, and dependencies;
display names are not truth or reusable proof identity. Different witnesses of
the same statement do not become one value. Forwarding and repeated projection
preserve the selected witness identity.

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

Normalize only operations licensed by that instance. Associativity,
commutativity, distributivity, and zero/one identities require their respective
laws; one does not imply another. Connecting nullary algebra operations to
constructor constants requires ordinary unfolding or citation. Search limits
and unequal normal forms do not establish inequality.

Closed proof-static indexes are canonical values. Open indexes normalize only
under the selected algebra and its checked public operation contract. Local
facts establish remaining compatibility without changing index identity.
Identity-bearing algebra evidence must be derived, not admitted. Retain the
algebra instance, normalized operation-contract identity, canonical expression,
compatibility evidence, and normalizer version. That version is provenance;
changing the canonical form is an explicit language compatibility event.

## Axioms and receiving policy

A theorem's meaning includes its calculus and exact transitive assumptions.
Axioms are selectable, not compulsory library truths. A dependency cannot accept
its assumptions on behalf of a consumer. An inconsistent assumption set can
prove false statements; derivation checking does not certify consistency.
Mathematical admission does not silently grant runtime-safety or artifact
acceptance authority.

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

A boundary claim that the checker can refute against declared ranges, domains,
or accepted statements rejects even when granted. This veto is not a proof that
the complete assumption set is consistent.

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

The separate [assertion design RFC](../../proposals/0006_assertions_and_build_diagnostics.md)
compares optional author-invoked diagnostics. It approves no assertion API,
source syntax, crash cause, build switch, or contract-widening mechanism.

Published proof evidence retains theorem identity, cited lemmas and premises,
normalization licenses, derivation/checker version, and trust/deferral closure.
Deterministic normalization owns identity; stronger proof search may establish
more claims but cannot rename them. Terminal obligations and certificate checks
follow the [verification contract](../terminal-psi/verification.md).

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

## Undetermined foundations

The following are required design work, not implicit choices made by current
syntax or the Rust checker:

- General logical-binder, predicate-abstraction/passing, and noncomputable-value
  source forms.
- The full dependent-function, universe, equality, computation, induction,
  quotient, and proof-irrelevance rules supporting general mathematics.
- Foundation identity and cross-foundation compatibility. Changing axioms within
  one calculus does not establish compatibility between different calculi.
- General derivation-record/small-kernel formats, any tactic-machine API, and the
  remaining Real/approximation library surface.

These open requirements do not authorize an arbitrary checker-plugin mechanism.
`PROOF-CONTRACT-MIGRATION` on the [execution board](../../../TASKS.md) owns worked
proofs and migration through source, serialization, and replay. Neither a
mechanical rewrite nor a few successful proofs establishes mathematical
completeness. Current automation belongs beside [validation](../../../omega-rust/psi/semantics/validation/README.md).
