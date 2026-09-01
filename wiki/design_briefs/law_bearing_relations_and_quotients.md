# Law-Bearing Relations, Evidence, And Quotients

Current design as of 2026-08-21. This brief resolves the former law-bearing
relations and quotients owner question. It
supersedes the current quotient pilot's executable-`bool` relation and
suffix-based discovery of `_reflexive`, `_symmetric`, `_transitive`, and
respect proof machines.

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
2. proposition-valued families over representative values;
3. carrierless selected-conformance evidence; then
4. relation-property conformances, quotient formation, and quotient lifting.

These items are not independently orderable. The systems dependent-contract
fragment remains separate: this ruling admits proposition-valued dependency
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

Heterogeneity belongs to the proposition telescope, not to a global role on a
carrier parameter. One proposition may bind independent `I` and `J` packs,
while another over the same carrier may use one shared pack and therefore
require identical indices. Declaring the proposition only forms that formula;
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

## Proposition families and evidence

A proposition family states a fact depending on representative values:

```omega
proposition rat_equivalent(left: Rat, right: Rat);
```

A nominal `converges_together<Left, Right>(left, right)` proposition publishes
`ConvergenceEvidence<Left, Right>` as its one opaque evidence interface:

```omega
proposition converges_together<machine Left, machine Right>(
    left: CauchySeq<Left>,
    right: CauchySeq<Right>
) evidence ConvergenceEvidence<Left, Right>;
```

The `evidence` clause is owner-controlled, fingerprinted signature content. A
witness-bearing proposition contains exactly one interface. The clause states
what an erased term of the proposition may project; it neither supplies a
producer nor exposes the selected producer in mathematical APIs.

Its truth is inhabitance by checked proof evidence, not the result of running
a `bool` machine. An arbitrary pair of generator machines cannot in general
be tested for convergence, but a package may exhibit a modulus and prove its
universal law.

`proposition` is a proof-formula declaration and generic binder kind. It has
no result, runtime value, layout, operation contract, lowering, or executable
body. A primitive fact-only declaration ends with `;`. A witness-bearing
declaration's `evidence` clause is its one canonical carrierless interface.
The owner thereby fixes both introduction and elimination: every establishing
conformance supplies that interface, and a named binding projects the exact
retained evidence term in the proof stratum.

The proposition remains nominal rather than definitionally equal to its
evidence interface. That interface is normalized, fingerprinted semantic content,
so changing that interface is a breaking proof-interface revision without
turning the selected conformance into the proposition's identity. An ordinary
`where` bound cannot carry this role, and `=` remains reserved for transparent
logical expansion.

An ordinary checked proof machine may establish either form through its
`ensures`. For a witness-bearing proposition the proof must also supply the
declared evidence. Relation-law contracts that already name the proposition
establish their applications by ordinary entailment; they need no separate
authorization route.

Naming a `requires` clause binds its exact incoming evidence term. Naming an
`ensures` clause declares an exact outgoing evidence term, definitely assigned
once on every outcome path where that clause applies. Member projection uses
ordinary `term.member` syntax; no `open` form exists. Producer conformance
selection occurs privately when the proof body assigns the named output, while
forwarding assigns an existing term and preserves its identity.

Proposition terms are always copyable. Consumable authority is an affine or
linear Type carrier, possibly with zero runtime layout, rather than a linear
proposition. An unnamed `requires P` imports only the fact; a named
`requires proof: P` retains one exact witness for projection or forwarding.
Naming changes the caller-facing proof lane and is therefore a breaking API
revision even though the required proposition is unchanged.

Trait machine requirements admit the same named lanes. The requirement owns the
ordered input proposition/interface rows and public output selectors; a
satisfier proves that exact surface and keeps its producer conformance private.
Incoming aliases are implementation-local, while changing an outgoing selector
is a breaking proof-API revision. Static and dynamic calls expose only the
opaque requirement-level witness declared by the trait, never a satisfier-
private proof identity or projection. This preservation is semantic only: no
evidence lane contributes runtime ABI storage or dispatch fields.

The first checked conformance rung implements that rule for one concrete,
non-generic satisfier and requirement. Named input aliases may differ locally,
but lane cardinality/order, normalized proposition, and carrierless evidence
interface must match; named outputs additionally retain the exact public
selector, with concrete strengthening appended only after the inherited
prefix. The inherited fact rows reuse the satisfier's exact checked evidence
terms and lane positions. Generic substitution, defaults, requirement calls,
dynamic dispatch, Terminal publication, and package exposure remain fenced
until their owners can retain the same identities.

The initial forwarding form is a bare-name assignment from a current machine's
named `requires` term to one of its named `ensures` terms. It erases before the
runtime statement stream and records an exact checked source-to-output binding;
both normalized proposition application and evidence interface must match. It
does not search visible facts or mint another witness. A concrete subjectless
conformance alias may instead introduce the output. That selection retains its
exact conformance and evidence-trait symbols, canonical instantiated
type-argument identities, and complete normalized realization rows. A wrong
argument or unresolved open endpoint rejects; no name-only or visible-fact
search participates. Producer assignment and path-sensitive outgoing definite
assignment stay separate. Definite assignment records each erased assignment's
exact statement coordinate and carries assignment state through the finite
named-state graph.
Every ordinary outcome must assign each outgoing slot exactly once; duplicate
assignment rejects, and a crash-only outcome has no outgoing proof lane. A
state-level named `requires` binds an exact arrival term, and every named
transition carries a separate erased identifier lane after `;`; checked binding
substitutes the target state's ordinary arguments before comparing proposition
identity. Machine-level incoming terms remain live across internal transitions
and are not redundantly repassed in the state-arrival lane.

Named requirements are positional erased proof inputs. Calls pass them
explicitly in clause order after a `;` separator, never through visible-fact or
conformance inference. The separator marks the boundary between ordinary Type
arguments and Prop inhabitants. Each position checks the supplied term against
the callee proposition after ordinary call-argument substitution; source and
callee binding names do not participate in matching. The separator remains in
an evidence-only call (`callee(; proof)`) and is omitted only when there is no
proof lane. Named guarantees are public proof-output selectors. Calls bind
selected outgoing witnesses after the
same `;` separator used by the input proof lane:

```omega
let (value; result_evidence: local_evidence) = produce();
```

The ordinary result retains its declared Type. A selected slot mints one fresh
caller-local term with the callee lane's exact proposition, interface, validity
scope, and provenance; an omitted slot contributes its fact but mints no term.
Selection is named because outputs are optional and selective, while incoming
requirements remain complete and positional. Proposition terms are copyable: a
bound term may be copied or forwarded repeatedly while valid, or remain unused.
An evidence-only binding leaves the Type lane empty (`let (;
result_evidence: local_evidence) = produce();`). The proof group is linked to
that exact checked call site and canonical terminal call operation; its runtime
effects, crashes, and fuel are only those of the ordinary call. No source-level
aggregate combines Type and Prop, and no generated output-package identity,
projection, lifetime, or partial-move rule exists. A generic producer
conformance is selected as a nested static application in the enclosing machine
telescope, for example `TogetherEvidence<Left, Right>`. Its type, `const`, and
static-machine arguments are explicit; ordinary lifetime elision alone may
omit a region, whose resolved identity remains checked and erased.

One exact result case may organize named and unnamed postconditions without
creating an output package:

```omega
ensures
    Result::Success -> {
        retained_evidence: P(result);
        Q(result);
    }
```

The arrow and braces attach ordinary guarantee rows to the exact nominal case;
the group itself has no value or identity. Named rows require one exact term
assignment on every ordinary exit producing that case. Unnamed rows require a
proof on every such exit and retain no source-bindable term. Both facts become
available only under the matching caller case refinement, whether or not the
named row is selected. Their validity scopes retain every referenced occurrence
and evidence-interface scope, so borrowing or revision invalidation composes
unchanged with guarded availability.

The first executable caller carrier permits one deliberately narrow
substitution: when a selected named proposition's only ordinary argument is the
callee's complete structural result, the guarded call retains the distinct
caller application plus a structured `(argument position, callee result,
caller result)` row. Terminal validation rejoins both applications to the same
declaration, binders, and evidence interface before guarded implication replay.
One bounded later-use successor permits the matching payloadless arm to pass
one, two, three, four, or five distinct selected terms once each as dense ordered
named requirements of one direct tail state. The tail accepts only the saved
whole result and returns it unchanged. Terminal retains the tail as a third,
verifier-resolvable machine plus one exact use row per term; the machine's
ordered requirement lanes, parameter/result shape, identity return, applications, interfaces,
terms, and input positions are replayed independently, without adding a runtime
call or fuel. Payload projections, six-or-more arguments, repeated term use,
later invalidation, and all wider call shapes remain fenced.

A transparent logical definition uses `=`:

```omega
proposition cauchy<machine Sequence>(value: CauchySeq<Sequence>) =
    converges_together<Sequence, Sequence>(value, value);
```

It expands before normalization and has no independent semantic identity. It
inherits the expanded proposition's requirements, trust class,
fact-or-witness classification, and evidence interface. Only its source name
survives for diagnostics and debug maps.

Proposition parameters state their kind and application signature explicitly:

```omega
trait Reflexive<C, proposition Relation>
where
    proposition Relation(left: C, right: C);
```

A `bool` machine application remains a valid contract fact: in a fact position,
bare `decision(a, b)` and `decision(a, b) == true` normalize identically. It may
back a transparent proposition definition when its proof expression is total
and otherwise fact-position eligible. Primitive propositions cover facts for
which no decision procedure exists.

The existential package reuses the complete conformance-row projection. One
requirement projection has two strata:

- a carrier-bearing, dynamically eligible machine contributes a runtime table
  slot;
- a carrierless machine contributes a stable opaque proof symbol plus its
  normalized contract;
- a law contributes its contract and never a runtime slot.

For example, convergence evidence contains a hidden modulus and its law:

```text
ConvergenceEvidence<A, B>
|- modulus(precision: Nat) -> Nat       opaque proof symbol
`- close_after(...)                    checked universal law
```

Projecting the same evidence term twice yields the same opaque symbol. Distinct
evidence terms may yield distinct proof symbols even when they establish the
same proposition. Proof irrelevance applies to proposition identity and runtime
representation; it does not make hidden witness terms definitionally equal
inside a proof.

A selected dynamic value may be owned by value when its *entire normalized
dynamic representation* has no runtime carrier. Absence of table slots alone
is insufficient: a runtime instance with no eligible methods may still have
unknown size and cleanup. Carrierless convergence evidence has neither an
instance nor slots, so an erased evidence term can retain an opaque selected
implementation without allocation, size/alignment metadata, cleanup, or a
source-visible conformance name.

Published mathematical APIs expose proposition names such as
`ConvergesTogether(a, b)` and transparent conveniences such as `Cauchy(s)`.
The underlying carrierless selected conformance is mechanism, not user-facing
mathematical vocabulary.

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

The proposition or other static subject of a law conformance is authored by
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

Fact-only versus witness-bearing classification does change proposition
identity, because it changes what a consumer may eliminate. Primitive
proposition symbols, binders, and that classification are fingerprinted in
terminal Psi. Transparent definitions are expanded and remain only in source
and debug metadata.

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

An admitted boundary axiom may state an environmental assumption elsewhere,
but it cannot establish the equivalence conformance or selected operation
theorem of a checked quotient. Quotient substitution is an internal logical
construction whose false equality would propagate without a containment
boundary. A relation
resting on environmental equality produces an explicitly axiomatized type
instead.

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
surface](../language_guide/chapter_5_expressions_evaluation.md#operators):
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

## Relation lifting through constructors

A relator supplies a proposition-valued `Lift` member. Its normalized form is
inherently heterogeneous:

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
checked evidence; admitted relation or operation-congruence evidence cannot
license `%`.

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
generic lift, and the proposition owner controls which elimination or
transport laws its proposition exposes. If an opaque proposition exposes no
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

The evidence producer checks the selected conformance. A consumer projects a
named published evidence term to artifact-local opaque symbols characterized
solely by the normalized contracts; witness implementation identity does not
cross the boundary. Terminal Psi currently serializes the forwarded subset as
dense evidence-term identities over exact proposition applications and
structured, source-handle-free interface identities. Forwarding preserves one
vocabulary row, and the verifier requires the term interface to equal the
application interface. Canonical positional contract-lane rows for the selected
terminal machine now reference the exact term IDs, with forwarding preserving
one ID across its required and ensured endpoints. Known IDs, dense positions,
and absence of orphan terms are verified. A producer-backed ensured term has a
separate canonical proof-bundle provenance identity retaining its exact
conformance, evidence trait, and complete normalized realization rows. The
verifier accepts an ensures-only ID only through one matching row; that row
changes proof identity, not semantic identity, runtime, or fuel. Each ensured
lane retains its public output selector beside the exact term ID; required
lanes have no output selector, and missing or duplicate names reject. The
interface also retains its complete normalized requirement surface.
`term.member` resolves to an exact checked term and requirement row, then
terminal Psi replaces the term handle with its forwarding-canonical ID and
retains the declaring trait application plus canonical requirement overload.
The verifier rejects unknown terms and rows. A dense source-coordinate-free
invocation table joins the normalized callee-machine identity to the selected
output lanes. Each selected public selector and callee term declaration binds
one distinct caller-local term with the same exact proposition and interface;
omitted lanes bind none. Repeated calls reuse callee declarations and their
producer provenance while minting distinct caller terms. A display spelling is
never an identity oracle. A generic conformance application retains its exact
declared name, complete normalized telescope including resolved elided
lifetimes, subject, trait application, and normalized row map. Expected
subject/trait shape validates this closed selection but does not infer its
non-lifetime arguments; visibility and ambient uniqueness never participate.

The first executable static requirement-call carrier is intentionally smaller
than that general model. An attached or free caller's explicit proof-static
binder may be specialized to one concrete named conformance whose direct
requirement and realization are non-generic one-state callables. Unit retains
its attached/free carrier. The bounded scalar rung admits only exact `i32` or
`bool`: its
specialized caller is free, and its requirement and realization are
receiverless with zero ordinary arguments. Erased named inputs do not count as
ordinary arguments. The scalar value crosses the ordinary call result; no
proof-specific runtime carrier is introduced. The
requirement may own any finite ordered set of subjectless named inputs,
including none, and must own at least one subjectless unconditional named
output; unnamed public rows remain outside this carrier.
Specialization retains the exact closed application and row before rewriting
the executable target. The selected row may be an inline realization or the
trait's exact default realization. Default reuse remains conformance-scoped:
each closed application retains its own commitment and generated realization,
and an inline override takes precedence. Checked call composition imports only
requirement-owned facts, and proof-output selection uses the requirement's
proposition, interface, lane, and public selector. The caller receives a fresh opaque term;
the satisfier's local alias, appended strengthening, forwarded term, and
producer provenance remain private. Terminal retains the normalized public
requirement separately from the owner-scoped application and concrete runtime
callee. Its closed callable registry separately commits the source-derived
matched requirement/realization result class, and verification rejoins every
ordered lane and runtime result with all four without
adding runtime arguments, storage, operations, or fuel. Inherited requirement
rows, generic or subject-bearing public surfaces, unnamed rows, direct
conformance-name calls, scalar results other than exact `i32` or `bool`,
receiver- or ordinary-argument-bearing scalar calls, attached scalar callers, and dynamic
dispatch remain closed.

Proof-only evaluation, when a transparent body is actually
needed, uses the ordinary gated build-time evaluator: semantic eligibility
requires the complete checked invocation contract and ordinary termination.
Deterministic work metering supplies progress, caching/accounting evidence,
warnings, and any
optional root-selected ceiling; long or unlimited evaluation remains legal.

## Migration and acceptance

The N6 formation boundary now accepts only a proposition relation and the
declaration's explicit named `Equivalence<C, R>` conformance. The selected
conformance is subjectless, closed, nongeneric, and exact; its inherited
Reflexive, Symmetric, and Transitive rows must have the canonical premises and
conclusions. The `Equivalence` interface itself must resolve to the sealed
toolchain declaration. Checked row dependencies are followed transitively, so
an admitted or boundary proof machine cannot hide behind a local value,
contract, guard, transition, or continuation. Generic relation applications
also retain and check their exact binder categories and order. There is no
Boolean-relation or structural proof-machine fallback.

The sealed-operation representation boundary remains narrower and
fail-closed. Typed calls retain the exact representative operation, exact named
theorem-machine application, and `lift`/`define` kind only for the sealed
`Quotient` spelling. The carrier is non-authoritative: no checked or terminal
operation is emitted until compiler-derived relations, correspondence, and
contracts are independently validated. The retired bare call pilot cannot
recover authority through structural proof-machine discovery.

The selected-theorem-schema verification rung is live. Planning derives the
exact expected ordinary parameter, `requires`, and `ensures` schema from the
closed representative application and relation plan, compares the explicitly
selected theorem after both closed static substitutions, and retains exact
machine/state contract-fact coordinates for every matched row. Extra or
missing premises, a finer or different relation, redirected or duplicated
representative calls, rebound shared arguments, parameter drift, named evidence
lanes, result-case/crash lanes, and conclusion drift reject independently. The
certificate adds no runtime dictionary and grants no executable authority;
general `lift` implication/correspondence and canonical Terminal retention with
independent replay remain fail closed.

The bounded `lift` correspondence certificate is also live for direct public
parameters, including explicit omission, permutation, and repeated
occurrences, plus contract-separable closed literals. Each public
occurrence row retains the actual public symbol at its
distinct representative position, and public `Q` dependency partitioning
consumes that map rather than declaration order. Repeated occurrences share one
exact instantiated value per theorem side without collapsing positional theorem
parameters, relation premises, representative applications, or legality
coordinates. A fact over an omitted formed-quotient parameter remains
additional dependent `Q`; a fact depending only on omitted ordinary parameters
remains fixed. Heterogeneous relation/type drift rejects. The certificate checks
structural `Q => P` inclusion separately after the selected theorem's left and
right representative substitutions, permits additional public `Q` facts and
reuse of one exact `Q` coordinate for multiple distinct `P` rows, and retains
the exact public, representative, and verified-theorem legality coordinate for
every included row. The non-executable certificate composes those rows with the
exact runtime correspondence and verified theorem. A closed boolean, an
in-range integer whose explicit suffix or exact concrete representative target
supplies the landing, or a float whose explicit suffix or exact `f32`/`f64`
target supplies the format may feed an immutable non-receiver representative
position when its primitive type and arithmetic domain or format agree. An
anonymous numeric scalar lands once at that exact target; the certificate
retains the derived integer width, signedness, and domain or float format. Exact
equality relates that ordinary input position; the literal value, spelling, and
landing remain runtime-evidence identity. A quoted raw-byte literal may also
feed an exact shared `&[u8]` representative position or an exact constrained
`[u8; N]` named value-domain buffer when its payload fits. The certificate
retains its immutable-image bytes and exact target identity; planning selects
no encoding domain and performs no mutable-view or different-buffer-shape
conversion. An exact-width quoted byte literal that ordinary contextual typing
has already landed as a canonical closed `[u8; N]` array may likewise feed that
exact representative target. The retained canonical shape is one unsuffixed
decimal `u8` value per element. Its ordered bytes and normalized array identity
remain evidence; quotient planning performs no padding, truncation, element
coercion, or contextual landing of its own. A direct closed Boolean array may
likewise feed only its exact literal-width `[bool; N]` target. Every element
must be a Boolean literal; values, order, and normalized array identity remain
occurrence and proof-substitution evidence. A direct fixed integer array may
likewise contain only integer literals at an exact literal-width primitive
`[I; N]` target. Each element independently follows the scalar landing rule:
its explicit landing must agree, or the exact element target supplies one, and
the value must fit. Ordered spelling/landing evidence and normalized array
identity are retained without element coercion. A direct fixed float array may
likewise contain only float literals at an exact literal-width `[f32; N]` or
`[f64; N]` target. Each element independently follows the scalar format rule;
ordered spelling/format evidence and normalized array identity are retained
without evaluating computed elements. A direct exact depth-two Boolean array
may likewise feed only its exact literal-width `[[bool; M]; N]` target. Every
row is a direct exact-width array literal and every leaf is a Boolean literal.
Row boundaries, ordered values, and normalized outer array identity remain
evidence. Proof-value array traces delimit every container, preventing a
nested array from colliding with a flat array carrying the same ordered
leaves. An exact depth-two fixed-byte array may likewise feed only its exact
literal-width `[[u8; M]; N]` target. Every row independently uses the canonical
fixed-byte rule: exactly `M` unsuffixed decimal `u8` leaves with no coercion or
computation. Ordered bytes, row boundaries, and normalized outer array identity
remain evidence. A direct depth-two fixed integer array may likewise feed only
its exact literal-width `[[I; M]; N]` target for a direct primitive integer `I`
other than `u8`. Every leaf independently follows the scalar landing and range
rule. Ordered spelling/landing/domain evidence, row boundaries, and normalized
outer array identity remain evidence; all `u8` matrices stay exclusively in
the canonical fixed-byte lane. A direct depth-two fixed float array may likewise
feed only its exact literal-width `[[f32; M]; N]` or `[[f64; M]; N]` target.
Every leaf independently follows the scalar format rule. Ordered spelling/
format evidence, row boundaries, and normalized outer array identity remain
evidence without evaluating computed leaves. A direct depth-three Boolean
tensor may likewise feed only its exact literal-width
`[[[bool; K]; M]; N]` target. Every plane and row is a direct exact-width array
literal and every leaf is a Boolean literal. Plane/row boundaries, ordered
values, and normalized outer identity remain evidence. Remaining exact
primitive fixed-array literals now close recursively: depth-three canonical-
byte, non-`u8` integer, and float arrays plus every exact primitive array at
depth four or greater retain every container boundary and the already-settled
leaf evidence in one recursive tree with normalized outer identity. Existing
flat, matrix, and depth-three Boolean owners retain priority; this fallback
neither reclassifies their evidence nor admits data aggregates. Exact
structural substitution can match a dependent representative `P` fact that
mentions a literal-fed parameter only when public `Q` contains the identical
post-substitution fact. Boolean value, integer spelling, landed type and
arithmetic domain, and float spelling and format remain proof-value identity
even where rendering is equal. When no exact fact match exists, one strict
authored-implication rung admits integer `ProofFact::Expression` goals that the
existing arithmetic contract engine proves from the complete ordered dependent
public-`Q` expression roster after exact left/right symbol,
representative-static `const`, and integer-literal substitution. Resolved
symbols, not display names, select atoms; only exact integer carriers/domains
participate, every hypothesis and goal must be inside the engine language, and
only `Proven` succeeds. Exact matches retain priority. Each arithmetic row
retains the full ordered public premise coordinates plus its representative
and distinct theorem-side legality coordinates for later replay. Unknown,
refuted, mixed membership/proposition, float, member-path, proof-view,
operator/domain, or identity-drifted judgments remain fail-closed.
Fixed representative call preconditions use a separate bounded certificate.
One exact substituted fixed-`Q` match, or one strict integer `Expression`
proof from the complete ordered fixed-`Q` roster, discharges the one
representative call performed at runtime; this proof is never duplicated into
two calls. Each row nevertheless retains both distinct verified theorem-
legality coordinates, so later replay cannot collapse the theorem's left/right
hypothetical applications. Direct resolved symbols, exact representative-
static `const` values, and exact integer literals are the only arithmetic
bindings, and only `Proven` succeeds. `define` permits no such weakening: its
fixed facts join the same exact one-to-one position/static-substituted `Q <=>
P` bijection as its dependent facts. Mismatched or
out-of-range integers, mismatched floats, mutable/non-byte, undersized, or
otherwise constrained byte-string targets, byte-string values not already
context-landed for a bare fixed array, noncanonical or heterogeneous
byte/Boolean arrays, mismatched or out-of-range integer arrays, mismatched or
computed float arrays, noncanonical byte matrices, mismatched, out-of-range,
or computed integer matrices, mismatched or computed float matrices,
noncanonical or mismatched recursive primitive arrays, ragged arrays, other
data nested arrays, other aggregates, zero-value,
casts, calls, computations,
constrained/generic
targets, mutable/attached targets, and every literal supplied to `define`
remain fail-closed. `define` remains strictly
position-preserving at exact public arity and continues to use its exact `Q <=>
P` bijection. Fixed facts without an exact match that require membership/
proposition transport, float or computed implication, a mixed premise roster,
unresolved identity, or argument adaptation remain fail closed, while generic
owner substitution, general adapted arguments, non-arithmetic logical
implication, and executable canonical Terminal replay remain fail closed.
Arithmetic `Expression` entailment is implemented. Checked planning for
quotient-domain membership and opaque proposition families now consumes the
settled explicit third static theorem application on `Quotient::lift`. It
verifies the exact left/right/shared parameter roster and the complete
fact-major public-`Q` `requires` and representative-`P` `ensures` rosters, with
adjacent Left/Right substitutions for each authored fact. Certificate
composition rejoins the exact `ForwardPreconditionTransport` role, complete
closed selected application, and checked-body, pure-closure, unconditional-
termination, and crash-free eligibility of both selected theorem entries. The
result is a distinct checked transport-backed lift certificate; automatic
implication and fixed-call rows are absent, while the theorem roster covers
both dependent and fixed `P`. The bounded non-executable Terminal carrier now
retains and independently replays its exact position-preserving direct form.
Every public-`Q`, representative-`P`, and congruence-legality row preserves the
Left/Right application side, authored source coordinate, and selected-theorem
coordinate; replay checks canonical fact-major and theorem-coordinate order and
joins the congruence `P` roster exactly to the transport `P` roster. Ambient
domain linking, visibility search, an opaque solver
verdict, or a mixed automatic/theorem row set cannot supply that authority.
The remaining lane is implementation work, not an open language-design
question.

A proof-only Terminal preparation seam now covers the total direct faithful
`define` shape and the position-preserving direct transport-backed `lift`
shape. Its all-or-nothing validation API produces one
source-handle-free aggregate containing package-qualified callable and type
identities, parameter ordinals, exact positional relations, theorem
parameters/premises/applications/conclusion, contract-fact coordinates,
purity/termination/crash eligibility, and the direct result edge. The seam
requires monomorphic one-state public, representative, and theorem machines;
empty static telescopes; immutable non-attached parameters; complete
eligibility; and unchanged direct result flow. Faithful `define` additionally
requires empty public/representative preconditions and no theorem legality
premises. The transport rung instead requires the
complete checked `Congruence, ForwardPreconditionTransport` roster and admits
no literal, omitted, permuted, repeated, generic, attached, or multi-state
adaptation. `TerminalModule` now retains those proof rows
in strict canonical-identity order, and its canonical codec includes each
aggregate and rederives its length-delimited identity on decode. Normal
representation validation independently reconstructs the structural theorem
and correspondence shape, re-encodes each identity, and rejects invalid,
duplicate, or reordered rows. The explicit proof-only Terminal producer
attachment consumes the extractor's complete batch transactionally and is not
an ordinary machine-lowering path. The rows still own no machine or
operation, normal validation continues to reject every quotient operation, and
a batch with one unsupported request yields no partial rows. This is a
source-erasure, module-retention, and replay prerequisite, not executable
stage-4 admission. A separate proof-only package-review projection now retains
the exact total, direct `define` correspondence. It transactionally rederives
the complete all-program extractor batch, requires exact batch equality, and
then selects a nonempty subset by the requested package's exact public
operation identity. The source-free canonical row retains the full theorem and
contract-fact coordinates, positional relations, eligibility, and direct
result coordinate; its explanatory source role names the public operation
declaration, not the compiler-synthesized typed call node. Package-review
schema 120 / row schema 78 / recovery schema 16 add the blocking row kind and
bounded opaque row recovery. Mixed-package extraction is permitted only by
filtering each review to its own rows; lift, adapted, private, unselected,
wrong-package, and batch drift remain rejected. Ordinary checked package
review still blanket-rejects quotient contract calls, so executable checked
operation/result lowering and the full package-review migration remain open.

The collection migration is now live through sealed typed requests, checked
relation planning, and the proof-only total-direct `define` plus bounded direct
transport-`lift` Terminal seam.
Three-argument `lift` retains `Congruence` followed by
`ForwardPreconditionTransport`. Its role-specific checked verifier now produces
the complete non-executable `Q -> P` transport certificate, and selected and
automatic transport are never mixed. Terminal representation replay is now
closed for its position-preserving form only. Package review blanket-rejects
quotient contract calls and has no quotient-operation record to migrate, and
execution admission remains fail closed.

The representation-observer fence is explicit at resolved-to-typed lowering.
A quotient cannot declare `Equatable`, participate as a field in synthesized
container equality, use runtime `==`/`!=`, or appear as the target of proof-only
`zero_value<T>()`; those surfaces would compare or choose retained
representative bytes without a checked law. Build-time layout/access schema
reflection rejects quotient roots and nested quotient record layouts instead
of exposing or inventing a zero-byte representative shape. Record and arm
destructuring also reject quotient subjects before field or case analysis,
including empty/rest patterns. Struct and case literals cannot mint a quotient
by naming its nominal type; only the exact carrier-to-quotient cast may
construct one. Equality in a proof-fact position stays as a logical fact for
the exact quotient-congruence judge and is never lowered to a structural
compare. Executable equality still requires a named lifted operation and its
separate `DecidesEquivalence` law.

For a request that is the direct terminal expression of a state, validation
now derives a non-authoritative relation plan when every selected quotient
relation is monomorphic. It records the exact quotient type and relation at
each quotient-bearing argument position, one shared binder occurrence at
every ordinary pass-through position, and the exact quotient result type and
relation.
Exact quotient type identity is retained so two quotients over the same carrier
cannot collapse. Indexed relation applications wait for the fully instantiated
representative-operation telescope rather than guessing independently
quantified binders from the quotient type. Except for the bounded closed-scalar
`lift` lane above, untyped or adapted arguments and nested result flow remain
unresolved and fail closed;
even a complete direct-terminal relation plan is rejected until operation
correspondence, the selected theorem contract, and normalized result flow
are independently checked and retained in checked/terminal identity.

The same non-authoritative plan resolves the selected representative entry by
its exact state symbol and retains its ordered runtime telescope, including an
attached receiver and excluding proof-static `const` binders, together with its
exact result and machine/state contract spans. Open generic/static applications
fail closed. A closed application retains its exact type, literal-`const`, and
static-machine bindings; an immutable structural substitution judgment applies
those bindings to representative runtime parameter and result types without
rewriting the checked type arena. This substitution judgment does not itself
substitute contract facts or validate the selected theorem; the separate exact
schema verifier consumes both closed applications. Faithful `define` runtime
correspondence remains the stricter direct shape below.

For the same direct `define` shape with a monomorphic quotient-facing owner,
validation now also derives a
non-authoritative runtime correspondence only when every authored argument is
the exact public parameter at the same position, every quotient parameter's
carrier (or ordinary parameter's exact type) matches the representative
parameter, borrow/mutable modes agree, multiplicity is preserved, and the
quotient result carrier matches the representative result. Attached receivers
participate by position and need not force the public parameter to be spelled
`self`. Reordering,
duplication, locals/constants, arity drift, borrowed quotient shells, and
carrier/result drift fail closed. Owner static/`const` correspondence,
contract-fact substitution, conditional/crash result flow, and the selected
theorem contract remain later obligations, so this direct correspondence
still grants no execution authority.

Representative static applications now have a separate non-authoritative
closure check. The selected entry's declaration telescope is paired exactly
and in order with explicit type, literal `const`, and static-machine arguments;
nested data/machine applications must themselves be closed. Wrong category,
arity drift, bare generic arguments, evidence projections, proposition-family
arguments, and lifetime-bearing applications fail closed. A valid generic
application is materialized by the non-authoritative checker as exact
parameter/argument pairs and is retained on the representative telescope.
Immutable substitution covers exact named/generic/reference/slice/array
runtime and result type identities, including literal substitution for array
length `const` binders; constrained, const-expression, and dynamic-trait shapes
pass only when already canonically identical. Contract propositions and owner
static correspondence remain unresolved, and no such plan can feed execution.

For a direct `define` with exact runtime correspondence, the non-authoritative
plan now also partitions both sides of the faithful-definition contract. The
quotient-facing machine and entry state contribute `Q`; the representative
machine and selected entry contribute `P`. A fact enters its dependent surface
when any expression position depends on a quotient-bearing public or
representative parameter at the corresponding runtime position. Facts
depending only on ordinary shared pass-through positions or ambient values stay
in that side's fixed contract surface. Expression, proposition-argument,
membership, receiver,
aggregate, indexing, and nested-call positions are traversed without
short-circuiting validation, and an unresolved value identity rejects the plan
rather than being classified as ambient. Exact side/owner/contract/fact
coordinates are retained. General proposition/static substitution, semantic
`Q <-> P` entailment, and executable certificate composition remain later
obligations.

The first exact `define` equivalence rung now consumes those partitions. It
alpha-renames the public and representative runtime parameters to their retained
position identities, then requires one order-independent bijection across the
dependent facts. Fixed facts remain ordinary call obligations outside `Q` and
`P`. Each match retains both side/owner/contract/fact coordinates. Expression, membership, and proposition
facts compare their closed structural identities after that positional rename;
missing, duplicated, category-drifted, or redirected facts reject. This proves
`Q <-> P` only when both sides are already the same normalized fact set. The
relation-plan coordinator delegates this entire dependency partition, exact
fact lookup, positional alpha-renaming, and bijection judgment to one focused
precondition module; the extraction changes neither the proof language nor its
admission order. Its exact public-parameter order, mode, quotient-carrier
matching, and representative static-substitution judgment likewise live in a
separate runtime-correspondence module rather than the coordinator. Exact entry
lookup, runtime telescope identity, and the shared-summary purity and
unconditional-termination certificates live in a third representative module;
it performs no local effect inference. General logical implication/equivalence
and executable certificate composition remain unresolved, so this evidence
still cannot admit execution.

The direct planning boundary recognizes a result root when the sealed request
is the call at the exact root of the state's last expression statement. It also
recognizes the same single edge through a complete straight-line chain of exact
immutable, result-typed local aliases: the request must be the first local
initializer, every intervening statement must directly name the preceding
local, and the state's final expression must directly name the last local.
Mutable or type-drifted locals, symbol reuse, nesting, unrelated statements,
assignments, transitions, and adapted expressions fail closed. Either accepted
shape records one unchanged fallthrough result edge.

One closed owner shape can now strengthen that edge without crossing the
fence. When the owner machine and state symbols are each exact-unique, the
machine has exactly that state, and the state contains no transition, the
non-authoritative plan records complete transition-free single-state normal-
result coverage. A duplicate owner identity, second state, or any transition
rejects this stronger certificate and retains at most the previously derived
fallthrough edge. A finite sibling-state graph can prove the same complete
coverage when every non-result state contains exactly one unconditional
ordinary named transition, every target is a unique state in the same machine,
and every path reaches the unchanged transition-free result state. The
certificate retains every exact source/target edge. Conditional or crash
transitions, continuations, extra statements, foreign targets, cycles, and
duplicate identities reject; validation performs one graph judgment rather
than adding a verifier for each hop count.

The rejecting request path also consumes the existing recursive operational
and service-reach fixed points. It retains an exact representative purity
certificate only when inferred transitive service reach is empty, suspension
and blocking are false, the representative telescope contains no mutable/out
parameter, and every concrete target in its reachable call closure resolves.
This is the shared whole-call-graph inference, not a second expression-local
effect analysis. Unconditional checked termination is retained independently;
progress-profile premises do not satisfy that fence. Neither result-flow,
purity, nor termination certificates prove contract correspondence, the
selected theorem clauses, custody preservation, checked/terminal retention,
or executable admission.

Acceptance requires:

1. primitive and witness-bearing proposition declarations retain their exact
   binder telescopes and classification, while transparent definitions expand
   without acquiring an identity;
2. carrierless evidence owns no runtime words and named terms project stable
   opaque symbols;
3. a proof of a witness-bearing proposition without its evidence rejects;
4. named proof inputs are passed explicitly in the erased call lane, named
   outputs are definitely assigned per applicable outcome and selectively
   captured through the erased output lane, and no visible-fact or conformance
   inference selects either;
5. proposition, evidence-term, and derivation-provenance identities remain
   distinct through forwarding, serialization, and checking;
6. a literally bodyless ordinary theorem machine rejects, an explicit boundary
   axiom is reported as admitted, and an equivalence depending on it cannot
   license `%`;
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
