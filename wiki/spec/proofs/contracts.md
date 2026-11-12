# Mathematical proof contracts

Omega's proof system uses ordinary machines, contracts, data, traits, and named
conformances. The [mathematical foundation](foundation.md) selects a dependent
core with proof-relevant identity and strict logical proofs. The
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
The body of each supplied theorem must prove it.

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
bridge. Changed argument values, observation-dependent inputs, different operation selections
or failed invocation admission invalidate that reasoning. This is call
congruence, not a general assertion that every comparison is reflexive.

A theorem-only machine has no Type result. Resultless law slots do not need dummy
returned witnesses. A Type-valued mathematical operation may instead denote a
value without an executable algorithm; it is not a resultless theorem or an
automatically executable call. The same checked contract may serve
runtime, compile-time, or proof use when its types and reach permit it. A fact-only
invocation emits no runtime call or work.

Law, lemma and theorem describe uses of ordinary declarations, not separate
source declaration kinds. In a trait schema, a requirement declared with
`machine` always contributes a selected callable field, including a resultless
requirement with `ensures`. To carry mathematical evidence instead, declare an
ordinary field of that evidence type or state a schema validity constraint.
Its identity follows the declared type, not the initializer's spelling or
runtime erasure; see [record elaboration](../language/conformances.md#implementations-are-operation-records).

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
total `embed` projection, retaining exact carrier range. This includes the
unbounded computational carriers `i` and `u`; it does not depend on a chosen
storage representation. Removing a policy
domain with `as` instead leaves Exact carrier arithmetic and retains
representability obligations. Trapping arithmetic, including the `!`
[policy operators](../language/numeric_values.md#policy-operators), does not
form a proof term; the normal return of a `!` operation establishes its
operation-specific mathematical denotation. For division that denotation is
integer quotient, not rational `/`. Floats project to
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
term to justify its own formation. Removing the policy domain with a cast or
`embed` selects the intended total reading; the two are not equivalent.

Integer/address embedding is injective within its exact source carrier and
preserves that carrier's range. It neither mutates the source nor creates runtime
storage. Computed operands preserve their selected policy: embedding a wrapped
sum does not turn it into unbounded addition. Subsequent mathematical operations
use their own declared meanings: `embed(7u32) / 2` is rational `7/2`, whereas
`embed(7u32 / 2u32)` is integer `3`. These are typed values, not a return to
anonymous literal inference. A call interpreted denotationally
needs a checked observation-free, reach-free, crash-free, terminating invocation,
not merely a same-spelled mathematical operation.

A generic proof using a static callable or conformance member must establish
those conditions from its originating requirement. Write `reaches none` when
the requirement must guarantee service-free invocation. Under
[deferred requirement reach](../language/effects.md#deferred-requirement-reach),
omission permits the selected implementation's row; today's pure selections
cannot justify proof use in an otherwise open generic body.

Builtin computational-integer equality and order, including `i` and `u`, have the same meaning as the relation
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

Named decisions `unbounded-computational-integers`,
`mathematical-exact-division` and `explicit-landing-proof-int-peer` are specified
under [numeric values](../language/numeric_values.md#unbounded-computational-integers).
Mathematical `/` admits `Nat`, `Int` and `Rat` by exact rational inclusions,
and anonymous operands by their complete exact rational value. Its result is
always `Rat`; the divisor's nonzero premise is established before forming the
term. Constant folding and expected result types do not change that result type.
An anonymous integral final value can land in mathematical `Int` or `Nat`
without fractional-history acknowledgment; a typed `Rat` needs explicit exact
narrowing for an integer-only use. Other mathematical promotions are not implied.

For admitted computational division, the proof correspondence is
`embed(a / b) == integer::quotient(embed(a), embed(b))`, subject to the source
operation's policy and formation conditions. Computational remainder similarly
corresponds to `integer::remainder`. The signed quotient truncates toward zero;
its remainder is zero or has the dividend's sign. These operations remain
available for number-theoretic reasoning; neither is mathematical rational `/`.
For an overflow-policy result, apply that policy to the operation-specific
integer denotation rather than to a rational division expression.

Each nested term retains its formation obligation: neither cancellation in
`x / x` nor multiplication by zero establishes an unproved nonzero divisor.
The desired conclusion cannot justify its own formation. The same semantics
apply to symbolic variables, generic selections and closed constants; a
constant-only arithmetic shortcut does not implement this contract.

For mathematical primitive result `M` and result-carrier bounds `[MIN, MAX]`,
the selected catalog supplies these denotations after primitive definedness:

| Policy | Law |
| --- | --- |
| Exact | Formation proves `MIN <= M <= MAX`; embedded result equals `M`. |
| Wrapping | Embedded result equals width-specific wrapping of `M`. |
| Saturating | Embedded result equals `M` clamped to the carrier. |
| Trapping | A normal return embeds to `M`; the exact primitive trap predicate governs failure. |

Here `M` is the selected primitive's integer denotation: for computational
division it is truncating quotient, even though source mathematical `/` returns
a rational. Unbounded `i`/`u` have no upper representability bound; `u` retains
nonnegativity. This table does not supply a modulus, finite endpoint or overflow
policy for an unbounded type.

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

Named decision: `stable-index-identity-and-compatibility`, ratified 2026-11-06.

Canonical index identity names declarations and specializations. Compatibility
evidence establishes that values fit a receiving indexed type. Keep these
judgments separate: a proof can establish equal indices without renaming
either application. This supersedes law-sensitive open-index identity and
bounded distributive expansion in identity.

The distinction applies to signatures, conformance applications, generic
argument inference, specialization keys, and value transfers. Exact declaration
matching uses canonical identity; value admission may additionally consume a
checked compatibility derivation. Neither failed search nor unequal canonical
forms proves mathematical inequality.

### Stable canonical index identity

Closed proof-static indices remain canonical values, obtained through ordinary
admitted evaluation under their exact concrete operations. Aliases expand
transparently. Open indices use a fixed, target-neutral profile of typed
expressions, with the following limited simplifications:

- Fold fully closed subexpressions through admitted evaluation.
- Flatten a same-operation chain only when that exact core operation's fixed
  profile licenses associativity.
- Sort operands by canonical encoding only when that profile licenses
  commutativity. Preserve multiplicity and all unlicensed grouping and order.
- Remove a correctly typed neutral literal only in the directions licensed by
  that operation's fixed profile. A chain consisting only of neutral elements
  retains the typed neutral value.

These rules are per exact core operation, carrier, and arithmetic policy, not
per token or primitive carrier alone. The profile is closed and published with
the core semantic contract; Build settings, imported theorems, trait extensions,
and a caller's available facts cannot extend it. User-selected operations keep
their exact applied operation/conformance identity and structural operands;
their algebraic laws support compatibility proofs, not canonical rewrites.
A wrapper does not acquire a core profile merely by forwarding to a core body.

For core integer addition and multiplication, Exact and Wrapping operations
permit the corresponding associative, commutative, and neutral-element
simplifications under their own meanings. Exact formation still proves every
authored intermediate representable; Wrapping retains the exact modulus and
carrier. Other policies require their own fixed licenses. For example, signed
Saturating addition is commutative but not associative: in `i8`,
`(100 + 100) + -100` is `27` while `100 + (100 + -100)` is `100`.
Neither a primitive operand nor a policy name grants a blanket semiring.
Trapping arithmetic remains subject to the existing proof-term admission rules.

Canonical nodes describe identity, not newly executed arithmetic. Do not use
key equality to erase or establish an authored expression's formation
obligations. Any newly introduced expression or call in an actual proof also
owes its ordinary preconditions and totality. Even a proof simplifying
`0 * (B + C)` cannot discharge an Exact overflow obligation on `B + C`.
Regrouping an encoding does not authorize executing a different partial
arithmetic expression.

There is no distributive expansion, factoring, user-law search, or expansion
budget in canonical identity. Symbolic `N + N` need not have the same key as
`2 * N`. A fixed profile must be deterministic and idempotent, and may flatten
and sort without multiplying expression terms. Sorting is not claimed to be
linear work. General compiler resource exhaustion can still stop compilation;
it must not return a different, partially normalized canonical key.

All producers share the canonical encoding and ordering. Retain typed leaves,
carrier/qualification identities, exact selected operations and applications,
and ordered children. Do not use arena handles, diagnostic aliases, debug
formatting, or hash-map iteration order as identity. Profile/schema versions
record semantic provenance; changing the canonical profile remains an explicit
semantic migration. Changing a proof-search budget does not change that profile.
An actual operation or published contract change remains an ordinary API change;
this rule removes proof discovery from identity, not those dependencies.

### Literal-linked index identities

The 2026-11-03 decision with this name is narrowed by
`stable-index-identity-and-compatibility`. Core-profile neutral-element removal
may identify `Indexed<N + 0>` with `Indexed<N>`. For a user algebra, the same
use is automatic proved compatibility when its checked laws establish it.

An abstract right-identity law
`core::Identity<T>(add(value, zero()), value)` combines with a checked
connection `core::Identity<T>(zero(), 0)`, with the literal formed in the exact
carrier `T`. Multiplication uses the corresponding identity and connection to
`1`. Each proof needs only its own laws, not a full ring or semiring.
Left and right identities remain separate. These user laws do not rename a
signature or specialization, even when their proof is automatic.
Removing a product with a zero operand separately requires its annihilation law;
unit laws and numeral spelling alone do not supply it.

Consume the complete selected conformance's direct and inherited law slots
under exact applications and substitutions. Recognize checked proposition
structure, not requirement names or punctuation. Match operation and nullary
slots by exact requirement identity, and never borrow another conformance's
law because the names agree. Conditional guarantees require their actual
premises at the use; no undischarged premise licenses an unconditional rewrite.
Generic reasoning uses the published contract without evaluating an abstract
implementation inside type identity. Concrete connection proofs may use ordinary
admitted computation or theorem citation.

### Proved index compatibility

Resolve types, arithmetic policies, and exact operation selections first.
Use the ordinary prover to establish equality of the actual and receiving
indices under those contracts. Core semantic facts, selected algebra laws,
established local equalities, and checked theorem guarantees may supply the
derivation. Automatic discovery must produce checked evidence. Importing a
theorem does not activate unrestricted global rewrite search.

For example, under an appropriate selected distributive operation:

```text
Buffer<A * (B + C)>
Buffer<A * B + A * C>
```

The open forms have distinct canonical keys, while a checked distributivity
proof permits value transport. No authored cast or proof call is required when
bounded automation succeeds. If it does not, an ordinary theorem citation may
supply the missing equality. Transport grants no unrelated predicate, ownership,
layout-recast, or access authority and inserts no runtime proof or conversion
solely for this index equality.

Selected user associativity, commutativity, literal connections, and
distributivity all operate here. Each transformation cites the exact operations
and law applications it uses. A law about one selected operation cannot rewrite
another. Internal repetition counts do not establish an abstract numeral's
meaning: `N + N = 2 * N` needs the relevant checked connection.

Mutual distributivity does not disable either valid law. There is no universal
expanded form to choose for declaration identity. The prover may apply a
direction needed for the goal, subject to its search resources and checked
premises; a cycle or failed search supplies no evidence. Adding the reverse law
cannot invalidate an existing derivation using the forward law.

The automatic expansion strategy defaults to at most 64 resulting summands
per attempted product expansion. This counts terms, not normalization steps,
integer magnitude, or total proof work. Count prospectively before allocating,
under the exact licensed operations, and record the measure and limit when
expansion is curtailed. Root Build may configure this search limit through
[recorded proof-work policy](../build/configuration.md#proof-work-policy).
It is not a type argument or an expression annotation.

A curtailed expansion does not reject a well-formed index by itself. When an
obligatory compatibility claim remains unproved, report that equality was not
established, naming the relevant search limit and possible theorem citation;
do not report that the types are mathematically unequal. A direct theorem can
apply distributivity with a large operand without expanding it.

### Exact matching and evidence transport

Proof compatibility does not merge declarations, choose an overload or
conformance, infer an otherwise ambiguous generic argument, or rename a
specialization key. A conformance targeting one exact symbolic form does not
automatically match another solely through a distributivity proof. Authors
requiring exact identity use a shared canonical expression or transparent alias,
or parameterize their declarations by the common index and state the required
relationships in contracts. Value admission can then consume evidence.

One generic conformance relates two index forms for a trait. Its telescope
takes both indices and the selected base evidence, and a
[`requires` clause](../language/state_contracts.md#requirement-clauses) on the
declaration admits the application; under that premise the members forward
arguments and results to the base through proved compatibility with no
conversion:

```omega
Reindexed<const A: u64, const B: u64, const Base: Ranked<Buffer<A>>>
satisfies Ranked<Buffer<B>>
requires A == B;
{
    machine before(left: &Buffer<B>, right: &Buffer<B>) -> bool {
        Base::before(left, right)
    }
}
```

Each remaining row forwards the same way. The adapter's members are new
declarations, so laws stated on the trait are obligations on them; they follow
from the base conformance's laws under the same equality, and a law observing
exact type identity may stay unproved, in which case the adapter rejects.
Selection stays explicit: a generic body under `requires A == B` selects
`Reindexed<A, B, Order>` by name, and no compatibility proof admits `Order` at
`Ranked<Buffer<B>>` on its own.

Different symbolic forms may create separate specializations. Fully closed
indices can still evaluate to the same canonical value and share a key.
Semantics-preserving code deduplication is a separate optimization, not a
promise of this identity policy. A stronger search setting cannot change
declaration selection or existing specialization identity.

Inside one compilation, retain the checked compatibility judgment through exact
generic substitution and lowering. Specialization consumes that judgment instead
of rediscovering the generic proof. It remains bound to its proposition, subjects,
selected operations/laws and premises.

Published Psi retains the judgment's semantic question, definitions and exact
dependencies. Under [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption),
host authorization of the trusted producer's exact artifact permits reuse without
rediscovery or certificate replay. A theorem identity or recorded success cannot
authorize that trust. A smaller receiving discovery budget does not invalidate
reuse; loading/preparation exhaustion remains a separate resource refusal.
Publication changes neither canonical identity nor declaration selection.

### Bounded canonical index normalization

The 2026-11-03 decision `bounded-canonical-index-normalization` and its
`mutual-distributivity-no-expansion` exception are superseded. This historical
anchor remains for existing links. The current rules are
[stable canonical identity](#stable-canonical-index-identity) and
[proved compatibility](#proved-index-compatibility); 64 is a configurable search
default, never a canonical-key cutoff.

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

Ordinary programs should reject this exceptional runtime relationship. A missing
proof, borrow-checker refusal, incorrect contract or compiler limitation is a
reason to repair the program or checker, not to request an assumed binding.
Libraries must not transfer these ordinary implementation failures into their
consumers' trust decisions. See the
[exceptional-use rule](../build/assumed_binding_contracts.md).

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

## Required automatic discharge

Named decision: `required-automatic-discharge`, settled 2026-11-10;
[the design draft](../../drafts/designs/required_automatic_discharge.md)
records the conformance work. An unproved obligation rejects, so what a
checker proves unaided decides which programs it accepts. Every conforming
checker therefore discharges one stated set of obligations, the floor, with no
cited lemma and no hint. A checker may discharge more; it may not discharge
less.

```omega
machine get(items: &[u8], i: u64, n: u64) -> u8
requires i < n
requires n <= items.len
{
    items[i]             // floor: i < items.len by order reasoning
}

machine area(count: u32, size: u32) -> u32
requires count <= 1000
requires size <= 1000
{
    count * size         // floor: the product is at most 1,000,000
}
```

The floor is defined by what is derivable, not by a search algorithm.

**Premises.** The facts valid at the obligation's program point under
[fact validity](../terminal-psi/fact_validity.md): the enclosing machine's
entry requirements, the default- and named-domain predicates of established
subjects, branch and pattern facts on the path, the guarantees of completed
calls, the exact definitions of locals, and each integer subject's carrier
bounds.

**Order reasoning.** An order atom compares two integer terms with `<`, `<=`
or `==`, where each term is a literal, a subject, or a subject plus or minus a
literal, read as mathematical integers. A subject is an exact value or place,
including a projection such as `items.len`. A goal that is an order atom, or
an inequality `!=` implied by a strict one, is in the floor when it follows in
integer arithmetic from the order atoms among the premises. The checker is
complete for this class: transitivity through any number of atoms, equality
substitution, adding one literal to both sides, the step between `a < b` and
`a + 1 <= b`, and contradiction of an unreachable path.

**Range propagation.** Each integer value defined by add, subtract, multiply,
divide, remainder, shift or cast receives bounds from its operands' bounds,
once, in definition order, by the arithmetic of
[`IntegerAffineBound`](../terminal-psi/integer_certificates.md#integeraffinebound).
An operand's bounds are the tightest literal bounds order reasoning gives it.
The resulting bounds are premises for order reasoning. A
[canonical scalar goal](../terminal-psi/integer_certificates.md#canonical-scalar-goals),
such as an exact operation's representability or a nonzero divisor, is in the
floor when these bounds establish it.

**Case splits.** A disjunctive premise whose atoms share a subject with the
goal is split, and the goal is in the floor when it is in the floor in every
arm, for up to 64 arm combinations per obligation.

**Outcomes.**

| Obligation | Result |
| --- | --- |
| In the floor | Accepted by every conforming checker under every proof-work policy. Failing to accept it is a checker defect, not a resource diagnostic. |
| An order goal that does not follow from the order atoms in scope | Rejected as "does not follow from the premises in scope", with a satisfying assignment of the premises that violates the goal. That is a statement about the premises, not about the program. |
| Anything else not established | Reported as not established. The author adds a premise, branches, cites a lemma, or relies on automation beyond the floor. Exhausted search is reported as a resource diagnostic. |

**Cost.** Order reasoning and range propagation are polynomial in the size of
the machine body, so the floor has no search allowance and is not governed by
the [proof-work policy](../build/configuration.md#proof-work-policy). Its work
is still charged as compiler work.

**Evidence.** A floor discharge produces the same kernel-checked certificates
as any other proof, from the
[elementary rules](../terminal-psi/integer_certificates.md#elementary-certificate-rules).
The floor fixes what must be found, not how it is represented.

**Beyond the floor.** General linear combinations, nonlinear facts other than
propagated ranges, and quantified or window facts are outside it. A checker
may still prove them, under the proof-work policy. Source that relies on that
is accepted only by checkers that find the proof, so a checker can be asked to
restrict automatic discharge to the floor with
[`--floor-only`](../build/command_line.md#checker-conformance), which reports
every obligation that needed more.

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

Local proof checking may use a kernel certificate: an object checked against
the exact question. Existing artifact encodings may carry such objects, but
receipt neither grants authority nor requires replay. Derive the review synopsis from that checked
object and its source-attribution metadata, never a second analysis of what
probably happened. It reports certificate identity, recursive components, implicit
closure rules, cited laws, and trust closure. The synopsis is explanatory; the complete
certificate and its checked question remain authoritative.

[Quotients](quotients.md) owns representative independence, selected theorem
roles, constructor relations, executable observers, and published correspondence.

## Delivery obligations

The [foundation](foundation.md) owns the selected mathematical model.
Binding, substitution, universe inference, checked logical evidence and
witness/assumption integration must follow the
[mathematical source forms](mathematical_bindings.md).

Changing axioms within one calculus does not establish compatibility between
different calculi. These source forms preserve the specified evaluation,
erasure, assumption-tracking and verifier-obligation rules. They authorize neither
an arbitrary checker-plugin mechanism nor retired formula-naming syntax.

Local proof representations, a tactic-machine API, and the remaining
Real/approximation library are separate engineering or deferred surface work;
they are not substitutes for implementing the selected foundation. Promote any
new semantic or trust choice they expose to the owner queue before relying on it.

The [W-based inductive profile](inductive_profile.md) and
[set-quotient interface](quotients.md#set-quotient-foundation) are selected.
Their guarantees require combined metatheory, encoding correctness, kernel
checking and library proofs. Neither a mechanical rewrite nor a few successful
proofs establishes mathematical completeness.
