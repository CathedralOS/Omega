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
conformances license quotient formation, and `Respects` licenses operations.
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
but it cannot establish the equivalence or respect conformances of a checked
quotient. Quotient substitution is an internal logical construction whose
false equality would propagate without a containment boundary. A relation
resting on environmental equality produces an explicitly axiomatized type
instead.

## Lifting operations: `Respects`

Equivalence licenses the quotient type; it does not license operations on it.
Every lifted operation needs a selected `Respects` conformance.

Normalize any machine's parameters, including an attached receiver, into one
ordered argument telescope:

```text
F : (argument_0, ..., argument_n) -> Result
requires P(argument_0, ..., argument_n)
```

The selected `Respects` requirement exposes two parallel copies of this
telescope. Positions are semantic identity; source parameter names are local
debug aliases and do not enter the fingerprint. Attached and free machines
therefore share one form without a generated global record name or an
author-declared adapter.

`RA` is the pointwise lift of the quotient relations selected for the
representative-bearing positions. It is never an arbitrary author-supplied
relation: a relation that is too fine could make both respect obligations
vacuous. `RR` comes from the requested lifted codomain relation, not merely
from the result type, because one result carrier may support several
quotients. `Respects<F, RA, RR>` proves two clauses.

### Domain invariance

```text
RA(x, y) -> (P(x) <-> P(y))
```

Equivalent representatives must agree on whether the operation is callable.
For a total machine `P` is true and this clause discharges structurally.
Only semantic preconditions depending on the representative participate.
Fixed ambient facts, authority, and resource requirements do not vary by
representative and remain ordinary machine-contract obligations.

The compiler finds the representative-dependent portion by semantic
dependency, not textual mention. A conjunct depending directly or indirectly
on at least one quotiented position enters `P`; a conjunct depending only on
ambient authority, capacity, or other fixed subjects does not. A conjunct
mixing representative and ambient subjects is conservatively
representative-dependent.

### Result congruence

```text
RA(x, y) && P(x) -> RR(F(x), F(y))
```

Domain invariance supplies `P(y)`. A failure represented as an explicit result
sum participates in `RR`; it does not need a second lifting mechanism.

For binary Real addition, `Arguments` contains both operands, `RA` is the
fieldwise product of `ConvergesTogether` for both places, and `RR` is
`ConvergesTogether`. Division additionally proves that equivalent
denominators agree about being zero. Comparison uses equality as its result
relation. The parallel-telescope normalization avoids an open-ended
`Respects1`/`Respects2`/`Respects3` hierarchy.

### Selecting a lift

The quotient owner selects the representative operation and one exact named
`Respects` conformance in an ordinary machine body. Two sealed core operations
make the distinction between a safe wrapper and a faithful definition
explicit:

```omega
machine Rational::divide(
    numerator: Rational,
    denominator: Rational
) -> Rational
requires
    denominator != Rational::zero
{
    Quotient::define<
        Fraction::divide,
        FractionDivideRespects
    >(numerator, denominator)
}
```

`Quotient::lift<F, Respect>(arguments)` is the ordinary compositional form. If the
enclosing path has public representative-dependent precondition `Q` and `F`
has representative precondition `P`, it proves `Q -> P`; the wrapper may
therefore advertise a strictly narrower domain. Constants, duplicated or
omitted arguments, reordered application, and computation around the lifted
result belong in this form.

`Quotient::define<F, Respect>(arguments)` requests a canonical quotient-facing
definition. It proves `Q <-> P`, and its normalized runtime argument telescope
must correspond position-for-position with `F`: every quotient parameter maps
to the representative at the same position, every non-quotient parameter
passes through unchanged, every representative position has exactly one public
parameter, and borrow mode and multiplicity agree. Type, `const`, static-machine,
and conformance arguments belong to the fully instantiated static application,
not this runtime correspondence. Constants, permutation, duplication, and
partial application reject with a suggestion to use `Quotient::lift`.

This is checked over normalized IR rather than source body shape. A
`QuotientDefine` result must reach every normal return unchanged, with no other
executable operation changing the result or adding an effect. Harmless aliases,
`let` bindings, and state forwarding do not alter that fact. The author chooses
`define`; adding a source-level alias cannot silently turn it into a wrapper.

Both operations derive `RA` from the quotient-bearing argument positions and
`RR` from the exact requested quotient codomain. Those relations are
compiler-owned typing operands and are never inferable authored arguments. The
selected conformance application is ordinary nested static syntax: all of its
type, `const`, and static-machine arguments are explicit, and only ordinary
lifetime elision applies. Visibility, priority, expected shape, and structural
proof-machine discovery never select it.

Checked and terminal identity retain the public quotient operation, normalized
representative-machine application, positional correspondence, `RA`, `RR`,
selected `Respects` conformance application, wrapper-versus-definition kind,
and the discharged contract-correspondence proof. Proof irrelevance permits
several public quotient operations to use different valid witnesses; it does
not permit the selected witness or provenance to vary by call site.

The initial `Quotient::lift` and `Quotient::define` operations accept only pure,
terminating representative machines whose observable contract consists of the
semantic precondition and normal result. Result congruence alone cannot show
that equivalent representatives perform the same I/O, take the same crash
route, suspend alike, or have the same progress behavior. Effectful lifting
requires a future relation over the complete observable behavior and does not
arrive by weakening this fence.

### Logical equality and executable observers

Logical equality on a quotient is induced by its selected equivalence relation.
It requires no executable decision procedure. Executable equality is an
ordinary quotient-owned operation, defined through `lift` or `define`, and is
unavailable until its named proof establishes `DecidesEquivalence`:

```text
equals(x, y) == true <-> R(x, y)
```

This soundness-and-completeness law is stronger than `Respects`: a
constant-false operation respects every equivalence but decides none.
Conversely, `DecidesEquivalence` plus the quotient's `Equivalence` proof derives
the ordinary `Respects` obligation, so the author never proves both. The
logical and executable uses consequently have one meaning; executable code
merely supplies a proved realization of it.

At the equality definition, the exact named `DecidesEquivalence` conformance
occupies the intrinsic's proof-selection position and the compiler records the
derived `Respects` bridge. There is no second witness-selection mechanism.

Quotient formation never binds this operation to the fixed `==` token. The
operation is an ordinary named declaration, and the token association uses the
general [fixed-operator declaration
surface](../language_guide/chapter_5_expressions_evaluation.md#operators):
`operator == Rational::equals(...)`. Callers may always use the named operation.

Other observer roles follow the same two-layer rule without sharing one false
generic law: `Respects` proves representative independence, while a
role-specific contract proves what the result means. Until a named role
interface exists, that semantic law is an ordinary checked contract on the
quotient operation. An ordering must justify its ordering claims, a canonical
representative must remain equivalent and be idempotent, and hashing requires
equivalent values to hash equally but never the converse because collisions are
legal.

### Fail-closed diagnostics

Diagnostics expose the failed semantic edge rather than reporting an opaque
quotient error:

- a missing lift proof prints the derived positional `RA`, requested `RR`, and
  exact expected named conformance application;
- failed wrapper admission distinguishes `Q -> P` from representative-domain
  invariance inside `Respects`;
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
checked evidence; admitted relation or `Respects` evidence cannot license `%`.

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
conformance application, and `lift`/`define` kind only for the sealed
`Quotient` spelling. The carrier is non-authoritative: no checked or terminal
operation is emitted until compiler-derived relations, correspondence, and
contracts are independently validated. The retired bare call pilot cannot
recover authority through structural proof-machine discovery.

Executable admission is currently blocked on the concrete `Respects` evidence
carrier. The source/core vocabulary has no sealed declaration capable of the
compiler-derived variadic positional telescope described above; authored empty
lookalikes are not authority, and an arity-indexed public trait ladder is a
rejected design. The checked/terminal evidence shape for that intrinsic
interface must be settled before the retained request can cross the boundary.

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
relation is monomorphic: `RA` records the exact quotient type and relation at
each quotient-bearing argument position and exact typed equality at every
ordinary position; `RR` records the exact quotient result type and relation.
Exact quotient type identity is retained so two quotients over the same carrier
cannot collapse. Indexed relation applications wait for the fully instantiated
representative-operation telescope rather than guessing independently
quantified binders from the quotient type. Untyped or adapted arguments and
nested result flow likewise remain unresolved and fail closed;
even a complete direct-terminal relation plan is rejected until operation
correspondence, the selected `Respects` contract, and normalized result flow
are independently checked and retained in checked/terminal identity.

The same non-authoritative plan resolves the selected representative entry by
its exact state symbol and retains its ordered runtime telescope, including an
attached receiver and excluding proof-static `const` binders, together with its
exact result and machine/state contract spans. Open generic/static applications
fail closed. A closed application retains its exact type, literal-`const`, and
static-machine bindings; an immutable structural substitution judgment applies
those bindings to representative runtime parameter and result types without
rewriting the checked type arena. This does not substitute contract facts or
validate `Respects`; runtime positional correspondence is derived only for the
direct `define` shape below.

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
`Respects` contract remain later obligations, so this direct correspondence
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
representative parameter at the corresponding runtime position. Facts depending
only on ordinary equal-by-`RA` positions or ambient values stay in that side's
fixed contract surface. Expression, proposition-argument, membership, receiver,
aggregate, indexing, and nested-call positions are traversed without
short-circuiting validation, and an unresolved value identity rejects the plan
rather than being classified as ambient. Exact side/owner/contract/fact
coordinates are retained. General proposition/static substitution, semantic
`Q <-> P` entailment, and the selected `Respects` clauses remain later
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
it performs no local effect inference. General
logical implication/equivalence and the selected `Respects` clauses remain
unresolved, so this evidence still cannot admit execution.

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
selected `Respects` clauses, custody preservation, checked/terminal retention,
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
9. a total respecting operation lifts;
10. a partial operation rejects unless equivalent representatives agree on its
   precondition;
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
