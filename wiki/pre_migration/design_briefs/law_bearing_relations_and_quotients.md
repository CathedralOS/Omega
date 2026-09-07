# Law-Bearing Relations, Evidence, And Quotients

> **Needs porting.** This document has not been consolidated or vetted for the
> current documentation structure. See the [migration index](../README.md).

Current direction: 2026-09-07. Relations are mathematical conditions, not
necessarily executable decisions. Proofs use ordinary machine contracts and
named trait/conformance bundles under the
[mathematical proof contract](../../spec/proofs/contracts.md).
General predicate naming and parameter syntax remain to be specified. The
schematic relation names below describe mathematical requirements, not a new
source declaration. Implementation migration remains open.

## Governing law

> A quotient relation is a proposition established by evidence, not a
> decision procedure. Forming a quotient requires a selected equivalence
> conformance. Lifting an operation requires a selected proof that both its
> domain and its result are independent of the representative. The retained
> representative is an implementation detail: no synthesized operation may
> turn its structural representation into observable quotient meaning.

The compiler knows no `Rat`, `Real`, Cauchy sequence, modulus, or convergence
rule. Core packages author those declarations and proofs through the general
surface below.

## Ordered dependency

Evidence-bearing quotients depend on a proof-side fragment of the
dependent-types ladder:

1. carrier families with typed, proof-static index telescopes;
2. general mathematical relation expressions over representative values;
3. carrierless selected-conformance evidence; then
4. relation-property conformances, quotient formation, and quotient lifting.

These items are not independently orderable. The systems dependent-contract
fragment remains separate: this ruling admits logical dependency
in the erased proof stratum, not arbitrary value-to-runtime-`Type`
computation, runtime proof objects, or value-directed layout.

The rational-carrier work also supplies the first rung of F7's float semantics.
Public `Rat` carries an `IntPair` numerator over a
positive `Nat` denominator, and `mk_signed_rat` canonicalizes the difference
pair before reduction. `rat_gap` remains `Nat`-valued; its reflexive, symmetric,
and shared-denominator triangle theorems were rebuilt over the signed
coordinates without changing their public statements. `FloatMeaning` and float
target providers must consume this one public carrier rather than grow a
parallel private rational theory.

## Carrier families and heterogeneous representatives

Every proof carrier participating in this facility is read as a family with a
typed index telescope, which may be empty:

```text
Rat                         index telescope = ()
CauchySeq<machine S>        index telescope = (machine S : Nat -> Rat)
```

Write an arbitrary instance as `C<I>`, where `I` denotes the complete index
pack. A binary relation over the family has the normalized shape

```text
R<I, J>(left: C<I>, right: C<J>) : Proposition
```

Relation subjects are representative values, never bare index symbols.
`CauchySeq<A>` and `CauchySeq<B>` may have different concrete types while
remaining instances of the same carrier-family identity. `Rat` is the
nullary case, so its index packs disappear. Quotient carrier matching uses the
family identity and rejects representatives from another family.

Heterogeneity belongs to the relation's logical binders, not to a global role on a
carrier parameter. One relation may bind independent `I` and `J` packs,
while another over the same carrier may use one shared pack and therefore
require identical indices. Naming the relation only identifies the formula;
checked evidence establishes its applications, selected relation-law
conformances license quotient formation, and explicitly selected checked
theorem machines license operations.
Carrier declarations consequently have no `index` or `phantom` relation
properties.

Structural lifting is conservative when no authored relation says otherwise:
corresponding static arguments must be identical. A heterogeneous proposition
or selected relator may instead bind distinct left and right packs and prove
the required relation. This keeps a static policy such as
`Encoded<Utf8>` distinct from `Encoded<Ascii>` by default without preventing a
different proposition from deliberately relating them.

The committed telescopes here are proof-static. This brief does not admit a
runtime-dependent carrier such as a vector indexed by a runtime value.

## Relations and witness bundles

A relation states a mathematical condition over its subjects. Some relations
are decidable by ordinary Boolean machines; arbitrary sequence convergence is
not. A contract can state the relation without promising such a decision.

For constructive convergence, a named trait bundle supplies a modulus and a
checked law about all positive precisions and indices beyond that modulus:

```text
ConvergenceEvidence<A, B>
  modulus: mathematical function from precision to an index
  close_after: checked law relating A and B beyond that index
```

This is schematic bundle content, not new source syntax. Ordinary machines
establish its laws. Named conformances select complete implementations, and
generic requirements let consumers carry one bundle instead of repeating each
witness and law separately.

A witness's identity and its proved properties are separate. Repeated use of
one bundle preserves the same modulus; two bundles proving convergence may
choose different moduli. Equal statements do not make those moduli
definitionally equal. Mathematical relation and quotient identity must not
depend on which conformance happened to supply a proof.

A constructive bundle is not the only way to state existence. Nonconstructive
existence under selected axioms must also be expressible. Its proof cannot be
silently extracted into an executable modulus. The calculus must specify
logical elimination, noncomputable values, and their erasure independently of
the ordinary bundle mechanism.

Trait contracts, call substitution, and Terminal replay must retain exact
subjects, bundle projections, path availability, validity scopes, and transitive
assumptions. Missing laws or a stale witness reject; matching names and visible
conformances cannot manufacture evidence. Source syntax for general predicates
and quantifiers is a migration dependency, not permission to encode them as
executable machine symbols.

## Relation-property hierarchy

Relation laws are ordinary explicit conformances. The compiler does not find
free machines by suffix or privileged global name. Each selected law
conformance is one closed implementation block; proof machines written in that
block or explicitly referenced by its trait-qualified rows supply the laws.
Bare exact-requirement satisfiers remain usable as proof lemmas but do not by
themselves form a selectable `Equivalence` conformance.

The reusable properties are independent:

```text
Equivalence<C, R>
|- Reflexive<C, R>
|- Symmetric<C, R>
`- Transitive<C, R>

Preorder<C, R>
|- Reflexive<C, R>
`- Transitive<C, R>

PartialOrder<C, R>
|- Preorder<C, R>
`- Antisymmetric<C, R>
```

Their normalized laws quantify index packs independently:

```text
Reflexive:
  forall I, x: C<I>.
    R<I, I>(x, x)

Symmetric:
  forall I, J, x: C<I>, y: C<J>.
    R<I, J>(x, y) -> R<J, I>(y, x)

Transitive:
  forall I, J, K, x: C<I>, y: C<J>, z: C<K>.
    R<I, J>(x, y) ->
    R<J, K>(y, z) ->
    R<I, K>(x, z)
```

`Equivalence` composes the three parent requirements and redeclares no law.
Several conformances may prove the same property. Selection always passes one
package-scoped conformance name explicitly; visibility, specificity, and
declaration order never choose it.
Changing the proof conformance does not change the nominal relation or quotient
identity.

The relation or other static subject of a law conformance is authored by
the law surface, never inferred from parameter occurrence. A truly subjectless
evidence interface uses the subjectless conformance form and receives a stable
package-scoped name; it is not forced onto an arbitrary parameter merely to
reuse a type-owned namespace.

The settled source form names the implementation first and owns its complete
telescope. A carrierless implementation simply omits the subject:

```omega
TogetherEvidence<machine Left, machine Right>:
    satisfies ConvergenceEvidence<Left, Right>
where machine Left(index: Nat) -> Rat;
where machine Right(index: Nat) -> Rat;
{
    // one closed row for every inherited requirement
}
```

It lowers to the shared resolved/typed conformance representation with an
explicit subjectless marker, a package-root `TogetherEvidence` symbol, and the
same exact normalized row keys used by carrier-owned blocks. Its inline
realizations have no attached data carrier. Trait arguments never infer the
telescope or nominate a carrier.

Normalized relation expressions, their binders, and their dependencies enter
semantic identity. Evidence bundle signatures and selected law dependencies
are retained separately. Presentation aliases and the chosen proof implementation
must not redefine the relation.

## Quotient formation

The quotient former consumes:

1. a proof carrier family `C`;
2. a proposition relation family `R` over heterogeneous instances of `C`; and
3. a selected `Equivalence<C, R>` conformance.

Conceptually:

```omega
data Real = CauchySeq % ConvergesTogether
where
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

The relation, not the selected proof implementation, enters quotient identity.
`representative as Quotient` is legal only for an instance of the declared
carrier family. Proven `R(a, b)` establishes equality between their quotient
images; quotient equality means membership in the same bucket, never
representative identity.

Quotient construction does not normalize. An implementation may retain the
chosen representative unchanged and give the quotient the same runtime ABI as
that representative. This is a zero-cost representation decision, not an
elimination rule: source code cannot recover, compare, hash, order, serialize,
reflect over, pattern-match, or otherwise observe the retained representative
unless a checked quotient operation licenses that observation. In particular,
quotient formation suppresses every synthesized representation-derived
operation, including structural equality. Struct and case literals cannot
construct the nominal quotient directly; casting an exact carrier instance with
`as Quotient` is the sole minting path.

The same rule governs compile-time evaluation. The evaluator retains the exact
representative chosen by construction, so ordinary const materialization may
emit it without canonicalization. The representation stays opaque, and
equivalent constants need not have equal bytes. A proved canonical form is
required only for representation-independent consumers such as stable
serialization, a public ABI representation, canonical const-index identity,
structural interning/hashing, or reproducible raw-byte observation.

The initial quotient surface also rejects carriers containing affine or linear
`Type` content or owned/routed custody. An equivalence that identifies distinct
authority, lease, root, or provenance occurrences would make those occurrences
substitutable and launder custody through logical equality. A future extension
may admit such a carrier only through a relation interface that preserves exact
custody occurrence; proof irrelevance and ordinary result congruence are not
that interface.

A mathematical quotient may depend on explicitly selected axioms, but its
equality then has those assumptions. A policy requiring assumption-free proof
rejects such a theorem. Only a consuming policy explicitly admitting the exact
dependencies may accept that conditional guarantee. No conformance wrapper can turn an admitted equality into an
assumption-free one. Foundation and axiom compatibility follow the general
mathematical proof contract, not a quotient-specific implicit grant.

## Lifting operations through selected theorems

Equivalence licenses the quotient type; it does not license operations on it.
Every lifted operation explicitly selects one representative machine and one
ordinary checked theorem machine. There is no `Respects<F, RA, RR>` interface,
variadic relation binder, arity-indexed trait family, ambient theorem search,
or runtime proof dictionary.

The theorem's ordinary parameters state the universal variables. A
quotient-bearing position appears twice, once for each possible representative.
A pass-through position appears once and the same binder is used in both
representative calls. `requires` states the selected quotient relations and
the legality of both calls; `ensures` states congruence in the exact requested
result relation:

```omega
machine fraction_divide_respects(
    left_numerator: Fraction,
    right_numerator: Fraction,
    left_denominator: Fraction,
    right_denominator: Fraction
)
requires
    FractionEquivalent(left_numerator, right_numerator);
    FractionEquivalent(left_denominator, right_denominator);
    left_denominator in Fraction::NonZero;
    right_denominator in Fraction::NonZero
ensures
    FractionEquivalent(
        Fraction::divide(left_numerator, left_denominator),
        Fraction::divide(right_numerator, right_denominator)
    )
{
    // proof
}
```

Both legality premises belong to the theorem. Its contract is checked
independently of any later consumer, and both calls in its `ensures` must denote
under its own `requires`. A quotient operation's public precondition does not
make a selected theorem well formed after the fact.

The compiler derives the expected theorem schema from the exact representative
machine application, quotient-bearing argument correspondence, selected input
relations, and requested result quotient. It then checks the explicitly named
theorem against that schema. Extra premises, a finer relation, a redirected
operation, duplicated or omitted representative, or separately rebound
pass-through argument reject. This is structural validation after explicit
selection, never structural discovery of authority.

### Public and representative preconditions

The quotient owner authors its public precondition `Q`. It is public signature
content and is never derived from the selected representative implementation's
precondition `P`; doing so would let an implementation change rewrite a public
contract.

`Quotient::lift<F, Congruence>` uses the compiler's complete exact/arithmetic
judgment to prove `Q -> P` for every representative application admitted by the
selected congruence theorem. A wrapper may therefore publish a stricter domain
and adapt, duplicate, omit, reorder, or supplement arguments explicitly. When
that bounded judgment cannot prove the complete implication, the owner writes
`Quotient::lift<F, Congruence, Transport>` and selects one checked resultless
transport theorem at the same operation request. The compiler derives the
complete ordered public-`Q` premise and representative-`P` goal schema,
including left, right, and shared parameter roles, and verifies the exact
selection against it. A selected transport is authoritative even if automatic
proof is available; there is no mixed theorem/automatic proof assembled row by
row.

`Quotient::define` is the faithful-definition form: it proves `Q <-> P`, exact
position-preserving argument correspondence, and unchanged result flow. For a
partial representative machine, the universally checked `Q -> P` direction
supplies legality for both equivalent representatives; no separately published
`P(left) <-> P(right)` law or source biconditional syntax is required.

```omega
machine Rational::divide(
    numerator: Rational,
    denominator: Rational
) -> Rational
requires
    denominator in Rational::NonZero
{
    Quotient::define<
        Fraction::divide,
        fraction_divide_respects
    >(numerator, denominator)
}
```

The distinction is authored, not recognized from source body spelling.
`define` is checked over normalized IR: every quotient parameter maps to the
representative at the same position, every ordinary parameter passes through
unchanged, modes and multiplicities agree, and the intrinsic result reaches
every normal return unchanged. Constants, permutation, duplication, omission,
or computation around the result reject with a suggestion to use `lift`.

### Theorem eligibility and identity

A selected theorem is proof-static authority, not a runtime call. It must be a
checked, pure, crash-free, suspension-free, blocking-free, terminating theorem;
admitted or boundary facts cannot license quotient substitution. The theorem
has no runtime dictionary, representative pair, fuel charge, or emitted call.
Theorem-only machines are resultless. A machine returns a `Type` result only
when that result is genuinely computed and observed in addition to its checked
contract.

An erased theorem citation is nevertheless a call-graph edge. If it enters a
direct or mutual recursion cycle, the cited contract may be imported as an
induction hypothesis only after that exact edge proves a strict decrease under
the component's well-founded ranking. Statement position, discarded result,
and value position use the same rule; an unmeasured self-citation cannot prove
itself.

Checked, package-review, and terminal identity retain the public quotient
operation, normalized
representative-machine application, positional correspondence, exact input and
result relations, lift/define kind, discharged contract/result-flow
certificates, and one canonically role-ordered theorem-evidence collection.
Every theorem entry carries its explicit `QuotientTheoremRole` discriminant as
an identity input, its exact selected application, a role-specific
correspondence payload, and the common checked-body, pure-closure,
unconditional-termination, and crash-free eligibility. Proof irrelevance
permits different quotient operations to select different valid theorems; it
does not permit selection or provenance to vary by call site.

The required-role set is closed by the authored operation form. Every operation
has exactly one `Congruence`; only the three-argument `lift` has exactly one
`ForwardPreconditionTransport`; current structural `define` has none.
Duplicate, missing, surplus, or noncanonically ordered entries reject. An
unknown role tag is a forward artifact-version incompatibility and always
rejects rather than being ignored by an older verifier. No reverse-transport
role is reserved until a theorem-mediated `define` with both implication
directions is designed.

The initial operations accept only pure, terminating representative machines
whose observable contract consists of the semantic precondition and normal
result. Congruence cannot show that equivalent representatives perform the same
I/O, take the same crash route, suspend alike, or have the same progress
behavior. Effectful lifting requires a future relation over complete observable
behavior and does not arrive by weakening this fence.

### Logical equality and executable observers

Logical equality on a quotient is induced by its selected equivalence relation.
It requires no executable decision procedure. Executable equality is an
ordinary quotient-owned operation, defined through `lift` or `define`, and is
unavailable until its named proof establishes `DecidesEquivalence`:

```text
equals(x, y) == true <-> R(x, y)
```

This soundness-and-completeness law is stronger than ordinary result
congruence: a constant-false operation is representative-independent but
decides no equivalence. `DecidesEquivalence` plus the quotient's
`Equivalence` proof entails the result-congruence theorem required at the lift,
so the author never proves both. The logical and executable uses consequently
have one meaning; executable code merely supplies a proved realization of it.

At the equality definition, the exact checked law machine from the named
`DecidesEquivalence` conformance may occupy the intrinsic's theorem-selection
position, and the compiler records the derived congruence certificate. There
is no second witness-selection mechanism.

Quotient formation never binds this operation to the fixed `==` token. The
operation is an ordinary named declaration, and the token association uses the
general [fixed-operator declaration
surface](../../language_guide/chapter_5_expressions_evaluation.md#operators):
`operator == Rational::equals(...)`. Callers may always use the named operation.

Other observer roles follow the same two-layer rule without sharing one false
generic law: the selected theorem proves representative independence, while a
role-specific contract proves what the result means. Until a named role
interface exists, that semantic law is an ordinary checked contract on the
quotient operation. An ordering must justify its ordering claims, a canonical
representative must remain equivalent and be idempotent, and hashing requires
equivalent values to hash equally but never the converse because collisions are
legal.

### Fail-closed diagnostics

Diagnostics expose the failed semantic edge rather than reporting an opaque
quotient error:

- a missing lift proof prints the expected theorem parameters, premises,
  conclusion, and exact selected theorem application;
- failed wrapper admission distinguishes public `Q -> P` correspondence from
  result congruence inside the selected theorem and, when the built-in complete
  implication fails, points to
  `Quotient::lift<F, Congruence, Transport>(...)`;
- failed faithful definition reports the `P -> Q` direction separately, or the
  first omitted, duplicated, permuted, constant, polarity-mismatched, or
  multiplicity-mismatched argument position, and suggests `Quotient::lift`;
- a result-flow failure identifies the normal exit or executable operation that
  prevents the `QuotientDefine` result from being returned unchanged;
- a representation-derived observer explains that arbitrary representatives
  may have different bytes and points to a named lifted operation;
- an executable equality proof distinguishes the soundness direction from the
  completeness direction of `DecidesEquivalence`; and
- an effectful, nonterminating, or custody-bearing request names the missing
  behavioral-respect or occurrence-preservation facility rather than silently
  weakening the quotient.

The maintained two-argument lift diagnostic now implements the built-in
implication case precisely. After the ordinary relation, runtime, and
precondition-partition judgments succeed, a failed dependent left/right or
fixed-call implication independently reconstructs those same partitions for
diagnostics. It prints the complete relevant public-`Q` machine/state contract
fact coordinates and the exact failed representative-`P` coordinate, then
names `Quotient::lift<F, Congruence, Transport>(...)` as the authored escape
hatch. That reconstruction is explanatory only and cannot mint a
correspondence certificate or execution authority.

## Relation lifting through constructors

A relator supplies a relation over the container and its component relation.
Its normalized form is inherently heterogeneous:

```text
Lift<I, J, R>(left: C<I>, right: C<J>) : Proposition
```

Instantiating both sides with one pack is the homogeneous case; it is not a
different relator. The container owner may publish several named, checked lift
policies, such as structural and unordered lifting. The quotient owner chooses
the exact policy for each `(quotient relation, container family)` use. That
selection is retained in semantic identity. There is no ambient default
relator and no conformance-priority rule; an uncovered pair rejects at
instantiation with the missing pair in the diagnostic.

For a transparent non-dependent product, the compiler derives the structural
lift recursively from the supplied field relations. An owner-provided coarser
lift is accepted only with a checked bridge showing that the structural lift
implies the chosen lift. An opaque constructor must publish the same bridge as
checked evidence. Any admitted premises remain visible to the consuming
policy; a checked-only obligation rejects them.

Dependent records lift in dependency order rather than as independent fields.
For example:

```omega
data Certified {
    root: RootId;
    proof [erased]: Authorized<root>;
}
```

The lift first relates the two `root` fields, adds that fact to the relational
environment, and then normalizes the two `Authorized<...>` applications. If
the roots are equal, the proposition applications coincide and proof
irrelevance discharges the evidence field. A coarser root relation requires an
authored transport theorem for `Authorized` under that relation.

The quotient owner must discharge that transport obligation because it chose
the coarser relation. The dependent type owner may publish a conditional
generic lift, and the mathematical interface determines which elimination or
transport laws are available. If an opaque proposition exposes no
sufficient theorem, the quotient owner cannot manufacture one and that lift
is unavailable.

An erased field remains part of this proof-side dependency analysis even
though it has no runtime representation. A lifted relation depending on
erased `Type` content is well-defined but has no derived runtime decider unless
checked evidence shows that content is determined by the runtime-relevant
projection. Requesting a decider without that evidence reports the exact
undetermined erased component. Proof irrelevance hides evidence identity only
after the proposition applications themselves agree; it never equates proofs
of different propositions.

## Cauchy construction

Core's landed metric theorem already proves the mathematical transitivity
step:

```text
close(p, q, 2e) && close(q, r, 2e) -> close(p, r, e)
```

Given evidence with opaque moduli `M1` and `M2`, transitivity defines the
symbolic witness

```text
M3(e) = max(M1(2e), M2(2e))
```

and proves its law from the two published laws plus the existing rational
triangle theorem. It never evaluates either hidden modulus at a numeral.
A transparent concrete modulus may still normalize when a separate proof
needs a concrete numeric result; evaluability is not required for quotient
transitivity.

## Separate compilation and resources

The evidence producer checks the selected conformance and its laws. A consumer
uses its declared interface, with exact normalized subjects, generic arguments,
and row selections retained through serialization and replay. The statement,
witness identity, and derivation provenance remain distinct. Forwarding preserves
the witness; separate witness introductions are not merged merely because
their statements coincide.

Proof-only bundles add no runtime words, table entries, allocation, or cleanup.
Erased Type components still retain their ordinary ownership and validity.
A call imports only the public guarantees actually established on that outcome,
not implementation-private strengthenings or facts from another result case.
Unresolved substitutions, orphan evidence, or missing laws reject.

Evaluation of a constructive witness uses ordinary checked invocation and
termination rules. It is not required merely to reason with the witness's law.
Noncomputable mathematical values cannot enter executable code through this
evaluation route. Metering and private resource limits govern proof work,
not the truth of an otherwise established theorem.

## Migration and acceptance

The implementation remains narrower than this design. `PROOF-CONTRACT-MIGRATION`
owns general contract logic and witness bundles; `QUOTIENT-THEOREM-LIFT` owns
executable lifting. Both are tracked in [TASKS.md](../../../TASKS.md).
Preserve the exact relation, selected conformance, role-specific theorem
identity, public/representative preconditions, and normalized result flow.
Do not infer a replacement predicate syntax from existing compiler carriers.

Acceptance requires:

1. normalized relation expressions retain exact subjects, logical binders,
   and dependencies without deriving truth from a name;
2. erased bundles own no runtime words and repeated projections preserve
   witness identity;
3. missing required witness laws reject;
4. call substitution and outcome checking preserve the bundle's public
   contract, validity scope, and exact witness without implicit selection;
5. statement, witness, and derivation identities remain distinct through
   forwarding, serialization, and checking;
6. a bodyless ordinary theorem machine rejects, accepted axioms remain
   disclosed transitively, and a checked-only admission policy rejects any
   law depending on them;
7. different family indices may relate, while a different carrier family
   rejects;
8. quotient formation requires explicit reflexive, symmetric, and transitive
   conformances through `Equivalence`;
9. a total operation lifts only through one explicitly selected resultless
   checked theorem whose exact ordinary contract proves result congruence;
10. a partial theorem states legality for both representative calls, while the
    quotient-facing author publishes `Q` and the lift proves `Q -> P` for each;
11. an operation whose result depends on representative choice rejects;
12. `Quotient::lift` accepts a checked wrapper after proving public preconditions
    imply representative preconditions, while `Quotient::define` additionally
    proves their equivalence, exact positional argument correspondence, and
    unchanged result flow over normalized IR;
13. quotient formation performs no normalization, exposes no representative,
    and synthesizes no representation-derived equality, ordering, hashing,
    serialization, reflection, or pattern operation;
14. executable equality requires a quotient-owned operation whose
    `DecidesEquivalence` proof is sound and complete, with any fixed `==`
    surface bound through the ordinary operator declaration head;
15. initial lifts reject effectful/nonterminating representative machines and
    carriers containing non-copy or owned/routed custody;
16. a generic conformance is selected through one nested application with all
    type, `const`, and static-machine arguments explicit, only ordinary
    lifetime elision, and no expected-shape or visibility inference; and
17. no compiler rule mentions `Rat`, `Real`, Cauchy sequences, moduli, or
    convergence.
