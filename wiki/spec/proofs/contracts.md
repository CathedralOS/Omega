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
by this model. [Optional naming syntax](../../proposals/proof_formula_syntax.md)
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

Published proof evidence retains theorem identity, cited lemmas and premises,
normalization licenses, derivation/checker version, and trust/deferral closure.
Deterministic normalization owns identity; stronger proof search may establish
more claims but cannot rename them. Terminal obligations and certificate checks
follow the [verification contract](../terminal-psi/verification.md).

## Published quotient correspondence

The retained Terminal correspondence table is proof-only. Its current certificate
forms are monomorphic total direct faithful `define`, and position-preserving
direct `lift` with `Congruence` and `ForwardPreconditionTransport` evidence.
Each row retains the exact public callable, selected application, theorem roles,
relations, eligibility, contract-fact coordinates, and direct result shape.

Role tags precede application and role-specific payload in identity. Transport
facts retain Left/Right application side, authored source coordinate, and selected
theorem coordinate. Rows are strictly identity-ordered; theorem evidence and
fact/source/theorem coordinates have canonical role-specific order. Decode and
validation rederive identity and the exact congruence/precondition-transport join.
Missing, duplicate, reversed, surplus, unknown-tag, or role/payload-mismatched
evidence rejects.

The table owns no executable machine or operation and authorizes no representative
call. Execution validation rejects nonempty tables until executable quotient
lowering is implemented. Proof-only package review independently reconstructs
the complete source batch; canonical retention is not ordinary checked package
projection or a substitute for source-to-executable correspondence.

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
