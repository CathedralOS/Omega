# Chapter 10: Compile-Time Proofs

Compile-time proofs use ordinary machines and contracts. A machine used only
to establish facts emits no runtime code. The basic judgment is:

```text
requires + body facts -> ensures
```

The checker checks the body under its declared assumptions. At a call, the
caller must establish those assumptions for the actual arguments before using
the conclusions. An unproved conclusion rejects unless it is an explicitly
accepted boundary claim. An unanswered proof obligation is not a proof of
falsehood, and returning an optional value is not a replacement for this judgment.

## Contracts and evidence bundles

A contract states a formula; a checked proof establishes it. A mathematical
formula need not have an executable decision procedure. Boolean expressions
are the decidable special case: a bare Boolean expression in a contract means
that its value is `true`.

Ordinary traits group operations, witnesses, and their laws. A named conformance
supplies the complete bundle. A proof machine can consume that bundle through
ordinary generic requirements rather than passing every witness and theorem
separately. The bundle does not establish laws merely by existing: its
implementations must satisfy the required contracts.

For example, the existing pointwise convergence model uses two sequence
generators, a modulus, and a theorem about values after that modulus. A
convergence bundle groups that modulus with the theorem. Composing two bundles
must prove the composed modulus's law. Merely calling a bundle
`ConvergenceEvidence` establishes nothing.

The source forms for general mathematical predicates, logical binders, and
noncomputable values remain design work. Do not replace them with a Boolean
decider or require every mathematical function to be an executable declaration.
The [mathematical proof contract](../pre_migration/design_briefs/mathematical_proofs.md)
defines the required capability and migration controls; the current Rust
implementation does not yet implement the whole replacement.

## Evidence identity and validity

Keep the statement, its selected witnesses, and its derivation provenance
distinct. Two proofs of the same statement do not make different witness
functions equal. Repeated projection of the same bundle retains the same
witness; forwarding must preserve that relationship and exact substitutions.

Contract facts and proof-only bundles add no runtime storage or calls merely
because they are used for proof. Type witnesses retain ordinary multiplicity
and validity even when erased. Logical facts cannot create or consume authority.

A conclusion applies only on the paths and result cases for which it was proved.
A statement about borrowed or revisioned data expires with its relevant scope
or an invalidating write. Trait satisfaction, call substitution, serialization,
and independent replay must preserve these conditions and the transitive
assumptions. Bundling must not erase them.

## Explicit relevance

Relevance belongs to a binding occurrence, independently of its type,
validity scope, and provenance. For a `Type` binding it is also independent of
that type's multiplicity. An erased field uses the same
bracket-property convention as other binding properties:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The checker retains `proof` in the typed and proof calculi but lowering gives
it no field offset, address, runtime read, or runtime cleanup. Erased data may
be consumed by contracts, proofs, other erased bindings, and statically checked
authorization for an effectful operation; it may not determine runtime data or
control. This noninterference rule is checked through every call and
projection. `[erased]` is therefore a relevance judgment, not a promise that
the implementation happens not to inspect the field.

A proof-machine result likewise exists only in proof computation. It may feed
another proof machine or an erased initializer, and a statement-position call
may cite the machine for its established facts, but its result cannot initialize,
return, branch, or otherwise determine runtime data.

Erasure does not discharge Type obligations. An explicitly erased Type ghost
may remain affine or linear, borrow-scoped, lease-scoped, content-bearing, or
provenance-bearing, and its obligations remain in the compiler frontier until
explicitly consumed. What is forbidden is a runtime destructor or cleanup
action that relies on erased representation. A containing value cannot leave
scope while an erased linear Type obligation remains live. Proposition proof
terms are different: they are intrinsically proof-only and always copyable.

A structurally zero-layout Type value needs no `[erased]` marker merely to cost
zero bytes. It remains an ordinary value and carries ordinary ownership and
multiplicity. Conversely, `[erased]` cannot be used to delete the bytes of a
representable runtime value; it is a checked relevance assertion for a
specification-only occurrence.

Construction normally supplies an erased term even though it produces no
runtime code. Omission is derived only for a structurally visible and
accessible nullary constructor; the compiler does not invoke a general
inhabitance judgment or synthesize a zero/default value. Ordinary runtime fields
retain their [construction and zero-establishment rules](chapter_1_data_values_literals.md);
data declarations do not permit field default initializers.

For currently resolved and nameable checked-shape holders and non-generic
evidence types, the executable slice elaborates an omitted erased initializer
only when exactly one payloadless constructor with no common fields determines
the term. Holders may also be closed synthesized generic records when the
instance is selected by an explicitly typed local initializer, direct exact
assignment, exact return, or one parameter signature shared by every
same-name free-call candidate. A syntactically direct `self.method(...)` call
uses the same rule over the exact enclosing attached-data owner; an explicitly
typed local receiver or direct `self.field` receiver uses its exact nominal
owner. The implicit receiver is not a value argument. Computed, chained, and
dynamic receivers remain fail-closed. This contextual record elaboration does
not infer type arguments from fields. Pure and mixed common-field/
case generic sums admit multiple exact closed instances per generic base in the executable slice. A
closed annotated local, direct assignment, agreeing free/direct-self call
parameter, or return destination selects construction identity; an exact local,
parameter, or attached-self-field subject selects
destructure identity. Other bare constructor contexts retain the unique-
instance fallback and fail closed when more than one identity is possible.
Nested concrete generic payload records reach the same synthesis fixpoint. The
semantic typed tree receives the constructor term
before proof and multiplicity checking. Ambiguous, absent, generic, or
otherwise ineligible evidence constructors still require an explicit term;
omission never invokes a default or general inhabitance search. Native erasure
admits non-generic transparent records, sums, mixed common-field/case shapes,
those closed generic-record instances, and the exact closed generic-sum cohort.
Closed plain records, sums, and mixed shapes may also have attached machines
when every attachment is an ordinary checked body with no unresolved machine
parameters. This includes
a closed synthesized instance of a generic record whose bodyful attached
machine is cloned and fully substituted for that instance; the generic template
itself has no runtime storage. Their machine storage and runtime contained-machine
topology use the erased-stripped fields, while semantic ownership and proof
obligations retain every field. Erased payloads do not
change tags or case numbering; they remain visible to semantic exhaustiveness
and obligation checking. The compiler fails closed for unresolved generic uses,
ambiguous generic record/sum construction contexts, computed, chained, or
dynamically selected receivers, placed views, wire/codec and ABI faces, and attached
machines over unresolved generic uses, non-checked supply modes, or unresolved
machine parameters.

Runtime layout, ABI classification, codec shape, and placement offsets use the
erased-stripped form. Nominal type identity and semantic fingerprints retain
the erased binding and its type. A placement gives an erased field no physical
offset; any fact it carries must instead be established by the checked or
admitted placement plan.

## Machines As Proofs

This machine proves a simple ordering fact:

```omega
machine distinct_indices(
    i: u64,
    j: u64
)
requires
    i < j
ensures
    i != j
{
}
```

The empty body is valid only if the checker can prove the guarantee from the
requirement and built-in arithmetic/order rules.

This machine proves a closed arithmetic fact:

```omega
machine pythagorean_3_4_5()
ensures
    3nat * 3nat + 4nat * 4nat == 5nat * 5nat
{
}
```

The checker reduces both sides to the same `Nat` value, then closes the equality
by reflexivity. The body does not need to simulate computation.

A theorem-only machine has no `Type` result. Its parameters state universal
variables, `requires` states hypotheses, and `ensures` states conclusions. A
machine returns a value only when it genuinely computes that value in addition
to proving its contract. Algebraic law slots and quotient-congruence theorems
are theorem-only; a dummy `-> Self` result is not induction evidence and must
not be required merely to carry a proof.

## Typed Facts

Proof facts must be typed.

```omega
3nat * 3nat
```

is math over `Nat`.

```omega
3i32 * 3i32
```

is machine arithmetic and carries machine obligations such as width and
overflow behavior.

The same operator spelling can exist in both worlds. The operand types decide
which proof rules apply.

Calls and projections in fact position are denotational terms. For example,
`add_int(a, b).pos` denotes the `pos` field of the pure call result; it creates
no runtime temporary, move, or loan. Such a call must be total and pure. A
fact's validity is the intersection of the validity and revision scopes of
every occurrence it references, transitively through those calls. An
intersecting write or revision transition invalidates the affected fact.

This does not transfer custody into `Prop`. A proposition may mention a linear
value or result, and copies of that proposition remain erased and copyable;
the actual `Type` occurrence retains its independent multiplicity and custody.
"No loan" here means no new runtime loan: a fact depending on an existing loan
still expires with that loan.

## Total Specification Arithmetic

Every term in `requires`, `ensures`, a domain predicate, or a guarded `crashes`
route is total. Exact arithmetic is admitted after its ordinary formation
obligations are proved. Wrapping and Saturating arithmetic remain admissible
after any primitive obligations outside their overflow policy are proved; for
example, neither policy makes division by zero a result. Direct Trapping
arithmetic is not a proposition term: its failure transfers runtime control,
and contracts do not execute.

A domain `requires` row must resolve to `Prop`. A machine returning `bool` is a
value term, not an implicit proposition and not a validator invocation hidden
inside qualification. Logical expressions may contain eligible total
machine calls. Substitution retains the exact called machine, argument terms,
and checked operational eligibility; an abbreviation cannot hide those
dependencies. No runtime call is emitted merely because a term occurs in a proof.

Use an explicit proof view when a contract needs unbounded mathematics:

```omega
requires
    embed(left) + embed(right) <= embed(i32::Maximum)
```

For every fixed-width integer and address carrier, `embed` produces proof
`Int`. Unsigned and address embeddings additionally establish nonnegativity and
their exact carrier upper bound. The projection has no runtime representation,
does not alter the source qualification, and cannot determine runtime data or
control. Floats instead use `Float::meaning32` or `Float::meaning64`, whose
`FloatMeaning` result preserves finite rational value, signed zero, infinity,
and NaN explicitly. Equality on that result is structural proof equality: the
single payloadless NaN case is reflexive, while positive and negative zero are
distinct. This is deliberately not IEEE `==`, whose NaN and signed-zero laws
are the opposite. The compiler retains the exact source carrier and recognized
projection contract for PCC, but emits no runtime projection or comparison.

Removing a policy with `as` is a different statement:

```omega
requires
    embed(right) >= 0
    embed(left) <= embed(i32::Maximum) - embed(right)
ensures
    result == (left as i32) + (right as i32)
```

The result expression uses Exact fixed-width arithmetic after the earlier facts
prove its intermediate result representable. An Exact operation cannot use the
proposition containing that same operation to justify its own formation, and
this is not shorthand for the unbounded expression above.

Proof `Nat` remains the natural carrier for induction, counts, and nonnegative
resource coordinates. Its ordinary subtraction is Exact and forms only when
the right operand is proved no greater than the left. Clamping is spelled
`Nat::saturating_sub(left, right)`; bare `Nat - Nat` never silently truncates.
An exact `Int as Nat` conversion similarly requires a nonnegative source.

Executable Trapping arithmetic independently creates a compiler-derived crash
site. A specification occurrence creates no crash edge. Authored `crashes`
routes are total may-ceilings, and coverage checks each derived guard `D`
against the authored alternatives `C_i` by requiring
`D implies (C_1 or ... or C_n)`. See
[Total Specification Arithmetic](../pre_migration/design_briefs/total_specification_arithmetic.md)
for the complete policy bridges and Terminal-Psi rules.

## Proof-Only Data

`Nat` and its kin are currently classified as proof-only: unbounded, with no
machine layout, no ZII obligation, and no runtime existence. Recursive data is
legal, and recursion is the present structural reason no finite layout can be
derived:

```omega
data Nat {
    case Zero;
    case Succ(n: Nat);   // recursive: no layout is derivable — proof-only
}
```

Working rules:

- **Proof-only is computed, never spelled.** A type is proof-only when it is
  recursive (directly or mutually) or any field's type is proof-only. There
  is no marker; writing recursive data is the opt-in, and diagnostics name
  the classification ("`Nat` is proof-only: recursive data has no layout").
- A proof-only value may appear **only in fact positions** — machine or domain
  `requires`, `ensures`, `where` clauses — and in proof-stratum machine bodies.
  It never has a size, an address, or a zero value.
- A machine whose signature mentions a proof-only type is itself proof-only:
  it is evaluated by the checker, never lowered.
- **The checker computes where values exist and rearranges where they do
  not.** `Nat`/`Int`/`Rat` facts evaluate with exact unbounded arithmetic
  (`3nat * 3nat` reduces to `9nat`); facts over axiomatized carriers such as
  `Real` normalize symbolically under the carrier's declared algebra. The
  operand type picks the mode.
- A pure, total, measured machine over ordinary machine types is **dual-use**:
  it runs at runtime *and* serves as a fact atom the engine reasons about.
  Most theorems about `u64` code cite dual-use machines directly and never
  need `Nat` at all; `Nat` appears when a claim is genuinely about unbounded
  mathematics.

The explicit-relevance migration preserves the structural fact while removing
the accidental universe split. A recursive or otherwise unlayoutable `Type`
may occupy an erased binding and participate in proof computation, but cannot
occupy a runtime-relevant binding. Explicit binding relevance takes precedence
during migration; the existing recursive-propagation rule remains legacy
inference for unannotated declarations until their surfaces are migrated.
Constructor choices count as representation as well as fields, so an
all-fieldless sum such as `bool` does not become erased by vacuity.

Core ships the roster: `Nat`, `Int`, `Seq<T>`, `Bag<T>`, and `Rat`. Every finite
nonzero float embeds into signed `Rat` exactly (binary values are dyadic
rationals), while signed zero, infinity, and NaN inhabit the separate
proof-level `FloatMeaning` cases. Float verification invokes executable
`FloatSemantics` functions whose finite branches are exact Rat arithmetic plus
one format rounding step. Its `FiniteNonZero` payload is `Rat::NonZero`, so
the proof carrier has no overlapping zero representation. `Int` is the uniform
proof embedding target for fixed-width integers and addresses. Its order has no
floor, so ranking views over it must produce a
well-founded `Nat` rank or carry a proven floor.

Builtin `Int` is already an integer type for
[typed integer quotient and remainder](chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder):
`a / b` truncates toward zero and `a % b` is dividend-sign remainder, both
requiring a nonzero divisor. Their results are unbounded; no machine-width
landing or runtime layout is introduced. In a proof with `a: Int`, `a % 2`
selects `Int` remainder, while literal-only `-3 % 2` still lacks a typed
operand and rejects. Exact unbounded evaluation does not reinterpret integer
division as rational division. This is the required contract; remaining
operator evaluation and proof support is tracked on the execution board.

Repeated projections of one exact contract parameter, result, Terminal value,
structural float leaf, or exact-bit literal denote one proof term only when the
verifier reconstructs the same format, projection operation, and recognized
core declaration/catalog contract. Authored source locations remain diagnostic
provenance. Distinct source terms require an explicit theorem and never become
equal from spelling, ordinary IEEE equality, or coincident runtime values.

Core's `Rat` stores a signed `IntPair` numerator and a positive `Nat`
denominator; `mk_signed_rat` cancels the pair's shared offset and reduces the
remaining magnitude with the denominator. Its Cauchy-facing metric still
avoids division. `rat_gap(p, q)` is the nonnegative absolute cross-product
numerator gap, and
`rat_close(p, q, precision) == Nat::Zero` states
`|p-q| <= 1/precision` by comparing `precision * gap` with the common
denominator in Nat's explicit saturating-subtraction order. Its reflexive and
symmetric laws are ordinary
checked machines; they are the metric substrate for the constructed `Real`
corpus, not compiler-known arithmetic.

The supporting natural metric is ordinary core code as well. `nat_gap(a, b)`
computes symmetric absolute difference from the two saturating-subtraction
directions, and
`nat_gap_triangle(a, b, c)` proves
`nat_gap(a, c) <= nat_gap(a, b) + nat_gap(b, c)` in the settled
`Nat::saturating_sub(left, right) == Nat::Zero` order spelling. Its proof uses
nested structural
case states; every value leaf is checked, and recursion remains admissible only
when strict-subterm provenance survives every state-parameter forwarding edge.
Proof citations are statement-ordered: an earlier checked citation can
establish a later citation's `requires`, but a later statement can never justify
an earlier call. No Nat metric law is built into the checker.

Rational triangle is likewise division-free. `rat_gap_triangle_scaled(p,q,r)`
lifts all three gaps to the shared denominator and proves
`q.den * gap(p,r) <= r.den * gap(p,q) + p.den * gap(q,r)`. It is an ordinary
composition of Nat gap homogeneity, commutative-semiring factor rearrangement,
and `nat_gap_triangle`. Citing it substitutes symbolic member places into the
consumer's frame (`p.den` becomes the actual argument's `.den`); the names in a
theorem declaration are never observable at a citation site.

The order layer used above is checked core code as well.
`mul_le_mul_right(a,b,k)` transports `a <= b` through a common multiplier;
`mul_le_cancel_right(a,b,k)` reflects the order when `k` is positive. The
first proof is requires-bearing induction: its induction hypothesis is visible
to an authored per-arm citation only when every premise instantiated at the
smaller self-call is already established at that statement boundary. Earlier
citations may make the conditional hypothesis available to later citations;
an unproved or membership-shaped premise contributes no hypothesis.

`rat_close_triangle_split(p,q,r,e)` is the reciprocal-precision triangle:
closeness of `p,q` and `q,r` at `e+e`, plus positivity of `q.den`, proves
closeness of `p,r` at `e`. It scales the denominator-shared gap triangle,
combines both premise bounds, cancels `q.den`, and then cancels the concrete
factor two. No division or hidden ordered-ring tactic enters the proof.

The first sequence-facing atoms are ordinary generic machines too.
`cauchy_at<Sequence, Modulus>(precision, i, j) == Nat::Zero` states the
same-generator point obligation after `i` and `j` have reached the static
modulus. `converges_together_at<Left, Right, Modulus>` states its
heterogeneous two-generator twin. Their arbitrary precision and index inputs
are the universal variables; positive precision and both modulus bounds are
ordinary `requires`. There is no hidden quantifier, runtime callable, or
compiler-known notion of convergence in this surface. Their same-generator
reflexivity and heterogeneous symmetry facts are checked generic theorem
machines and remain citable at concrete generator/modulus selections.

`converges_together_at_triangle_split<Left,Middle,Right,Modulus>` lifts the
doubled-precision Rat theorem to one shared middle index. Both precision levels,
all modulus thresholds used by the premises and conclusion, and the actual
`Middle(index).den` positivity fact remain explicit requirements. Static-machine
application member places preserve the selected generator during citation
substitution; a positivity fact about another generator does not alias it.

The pointwise corpus supplies laws that a convergence bundle must establish.
The remaining proof-language work packages a modulus with its universal law
and supports existential claims without demanding executable witness extraction.
These are distinct capabilities. The required general relation expressions,
typed index telescopes, and bundle contracts precede full quotient formation.
See [Law-Bearing Relations, Evidence, And Quotients](../pre_migration/design_briefs/law_bearing_relations_and_quotients.md).

A quotient coarsens a type: sort its values into buckets of things a
proven equivalence calls interchangeable, and the buckets become the
values — read `%` as it already reads everywhere else, modulo. Wrapping
arithmetic is the familiar instance: `u32` addition is integer addition
with numbers differing by 2^32 counted the same.

```omega
data Real = CauchySeq % ConvergesTogether;
```

This is the bodyless `data` declaration (the `const X = ...;` shape): the
right side is a type expression, and `%` is its one new form. `CauchySeq` is a
proof carrier family whose typed index telescope contains its generator
machine. `ConvergesTogether(a, b)` is a proposition over representative
values. Its representatives may be `CauchySeq<A>` and `CauchySeq<B>` with
different generator indices while sharing the same family identity. Rat is the
same model with an empty index telescope. Quotient carrier matching never
admits an instance of a different family.

A constructive convergence bundle groups a modulus with a checked universal
closeness law. It is ordinary mathematical evidence organization, not a
mandatory wrapper around every statement. General nonconstructive existence
must also be expressible under explicit assumptions. The source spelling for
naming the general relation remains to be specified; the quotient examples here
use mathematical schematic names, not an implicit executable decider.

Relation properties are ordinary explicit conformances. `Reflexive`,
`Symmetric`, and `Transitive` are independent requirements;
`Equivalence<C, R>` composes all three and redeclares none. Preorders and
partial orders reuse the same component properties. Law evidence is selected
through those conformances rather than discovered from proof-machine names.

`%` consumes the carrier family, proposition relation, and one explicitly
passed `Equivalence` conformance. It never searches visible conformances or
selects an individual law satisfier. Quotient formation remains
carrier-only (`seq as Real`; `42 as Real` does not compile — that road runs
through `Rat` and a constant stream). Proven `ConvergesTogether(a, b)`
establishes logical equality between `a as Real` and `b as Real`. Equality on
the quotient means "same bucket," never "same representative". This logical
fact does not synthesize an executable structural `==` operation.

The quotient declaration names that evidence in its static `where` surface:

```omega
data Real = CauchySeq % ConvergesTogether
where
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

Here `as CauchyEquivalence` references an existing named conformance; it does
not declare one and does not enter quotient identity.

Every path in this formation surface remains an authored declaration
selection. The carrier, quotient relation, repeated `where` relation, sealed
`Equivalence` trait, and trait arguments follow the quotient data declaration's
visibility. The selected conformance is private formation custody rather than
quotient API identity, but selecting it across a package boundary still
requires an ordinary direct dependency and a public conformance declaration.

Equivalence licenses the quotient type, not operations on it. A lifted
operation explicitly selects the representative machine and one ordinary
checked theorem machine. The theorem's parameters state its universal
variables: quotient-bearing positions appear as left/right representative
pairs, while each ordinary pass-through position is one binder reused in both
calls. Its `requires` names the exact selected relations and makes both
representative calls legal; its `ensures` states congruence in the requested
result relation.

For example, a partial representative operation proves its theorem under both
call preconditions:

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

The compiler derives that expected contract from the representative operation,
public-to-representative argument correspondence, selected quotient relations,
and requested result quotient. It validates the exact named theorem after
selection. It never discovers a theorem by visibility or shape, and no
`Respects` interface, variadic proof binder, arity-indexed trait family, or
runtime dictionary exists.

The quotient owner selects both machines in an ordinary body through one of
two sealed core operations:

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

The quotient owner authors public precondition `Q`; it is never derived from
the implementation-scoped representative precondition `P`.
`Quotient::lift<F, Congruence>` is the wrapper form when the compiler's complete
exact/arithmetic implication judgment proves `Q -> P` for both representative
applications. Arguments may be adapted explicitly and `Q` may be stronger.
When that bounded judgment is insufficient, the quotient owner instead writes
`Quotient::lift<F, Congruence, Transport>` and explicitly selects one checked
resultless theorem proving the complete ordered `Q -> P` obligation. A supplied
transport remains the selected proof route even if a later compiler could prove
the implication automatically; the compiler never mixes automatic and theorem
rows. A failed built-in implication points to the three-argument form and prints
the unmatched public and representative fact coordinates.

`Quotient::define<F, Congruence>` is the faithful-definition form:
`Q` and `P` must be equivalent, runtime arguments correspond
position-for-position with no constants, permutation,
duplication, or omission, and the intrinsic result reaches every normal return
unchanged. The compiler checks these facts over normalized IR, so aliases and
state forwarding do not change the classification. A rejected `define` points
to `lift` when the body is an honest wrapper.

For a partial operation, universally proving `Q -> P` supplies legality for
both equivalent representatives. No separate domain-invariance law or source
biconditional is required. Both legality premises remain in the selected
theorem itself because its calls must denote under its own contract,
independently of any later lift.

Each selected theorem is resultless proof-static authority. It must be checked,
pure, crash-free, suspension-free, blocking-free, and terminating. Selecting it
emits no theorem call, proof object, representative pair, or dictionary.
Checked and terminal identity retain the operation, correspondence, exact
relations, lift/define kind, contract/result-flow certificates, and a canonical
role-keyed theorem-evidence collection. Every entry retains its explicit role
discriminant as an identity input, exact selected application, role-specific
correspondence, and shared eligibility. `Congruence` is always present;
`ForwardPreconditionTransport` is present exactly for the authored
three-argument `lift`. Duplicate, missing, surplus, reordered, or unknown roles
reject. An older verifier must reject an unknown role tag rather than skip a
newer proof obligation. Current structural `define` has no transport role, and
no reverse role is reserved.

Implementation remains incomplete. Full formation and executable lifting must
preserve the exact relation, selected law conformance, theorem applications,
contract correspondence, and result flow through Terminal Psi and replay.
Unsupported cases reject; a matching name or shape cannot supply missing proof.
The implementation migration is tracked in `PROOF-CONTRACT-MIGRATION`, with
executable lifting tracked by `QUOTIENT-THEOREM-LIFT`.

A quotient may retain an arbitrary representative unchanged at runtime and may
therefore share its ABI without performing normalization. The representative
is nevertheless opaque. Quotient formation suppresses synthesized structural
equality, ordering, hashing, serialization, reflection, pattern matching, and
every other representation-derived observer. Struct and case literals cannot
forge a quotient value; casting an exact carrier instance with `as Quotient` is
the sole construction path. Logical quotient equality is the declared
relation. Executable equality is an ordinary lifted quotient operation and must
prove `equals(x, y) == true <-> R(x, y)` through
`DecidesEquivalence`; that law also entails the required result-congruence
theorem. The named operation may bind fixed `==` through the ordinary
[`operator` declaration head](chapter_5_expressions_evaluation.md#operators).
Other observer roles state their role-specific correctness as ordinary
contracts until a named interface exists.

Compile-time evaluation preserves the exact representative supplied by quotient
construction. An ordinary `const` may therefore materialize that carried
representative without proving a canonical form; this is no stricter than
runtime construction and grants no new observer. Canonicalization is required
only when a consumer asks for representative-independent identity, including a
stable serialized/wire form, public ABI promise, canonical const-index atom,
structural interning/hashing, or reproducible raw bytes. Equivalent quotient
constants may otherwise contain different opaque representatives.

Initial lifting is deliberately pure and terminating. Observable effects,
crash routes, suspension, blocking, and progress behavior need a richer
behavioral respect relation and cannot be justified by result congruence alone.
Likewise, initial quotient carriers contain no affine/linear `Type` content or
owned/routed custody: quotient equality may not make distinct authority or
provenance occurrences substitutable.

Carrier declarations do not assign global relational roles to their static
parameters. A proposition may quantify independent left and right index packs,
or use one shared pack, according to the relation it declares. A selected
constructor relator is correspondingly heterogeneous, `Lift<I,J,R>`, and the
quotient owner chooses the exact named lift for the known quotient/container
pair. The exact selection is retained in semantic identity, and an uncovered
pair rejects at instantiation.

Transparent non-dependent products lift recursively. Dependent fields lift in
dependency order: facts established for earlier left/right fields determine
whether later proposition applications coincide or require an authored
transport theorem. The quotient owner discharges transport required by its
chosen relation; its published mathematical interface determines which laws
are available, not the name of a hidden implementation. A relation depending
on erased `Type` content remains proof-only
unless checked evidence shows that content is determined by the runtime
projection, in which case a runtime decider may be derived.

An attached proof-carrier operation used this way has a by-value receiver and
does not install a representative-facing method or reify a representative on
the quotient. A borrowed or mutable receiver remains a forbidden runtime use
of proof-only data. Copyable runtime carriers may receive pure executable
quotient operations through the same sealed lifting gate; the representative
still never becomes source-visible.

Checked quotient formation and lifting require proofs of their exact laws.
Any admitted premises remain in those proofs' transitive assumption closure.
A policy requiring assumption-free quotient safety cannot accept a proof that
depends on such premises. General mathematical reasoning under selected axioms
must not be confused with an unconditional artifact guarantee.

A literally bodyless free machine is not a theorem. Checked theorem machines
have bodies, including an empty `{ }` body when their conclusions follow from
entry facts. An accepted axiom is an explicit boundary claim and retains
admitted provenance. Neither a bundle name nor an unavailable witness can
establish an otherwise unproved conclusion.

## Proof Views

`embed(value)` is a compiler-owned fact-position term former with canonical
semantics, like `old(&place)`. It is not executed, overridden, selected, or
declared as a bodyless boundary machine. A package-declarable proof-term-symbol
surface, if ever justified independently, must be designed explicitly rather
than inferred from the temporary Real scaffold.

Runtime data often needs a mathematical view before it can be reasoned about.

For slices, useful proof views include:

```text
Seq(items)    ordered finite sequence view
Bag(items)    finite multiset/counting view
Range(len)    finite index space
```

These are ordinary proof-only types from core — recursive data plus
extraction lemmas, not compiler-known forms. They do not allocate at runtime;
they let contracts talk about math without pretending that proof binders are
runtime loops.

`Sorted` is an ordinary domain defined by a predicate machine (see Quantified
Facts below); the views exist so contracts can talk about order and counting
without inventing runtime loops. Sorting is naturally expressed as:

```omega
machine Sort::bubble_sort_preserving(
    before: &[Nat],
    items: &mut [Nat]
)
requires
    Bag(items) == Bag(before)
ensures
    Seq(items) in Sorted
    Bag(items) == Bag(before)
{
}
```

The `before` value is explicit. A caller that wants to preserve an arbitrary
computed value can make or carry such a snapshot itself.

Contracts also have the narrower proof-only `old(place)` form. It selects the
callable-entry revision of a structural place so a postcondition can relate
that place's prior and current content. It is not a runtime snapshot, does not
duplicate the place or its value, and initially does not accept an arbitrary
computed expression. For example, `old(&extent)` gives a content projection a
stable pre-state subject while preserving the exact owned occurrence.

`old` is derived from the same place-revision model used by scoped facts and
borrow certificates; it is not a second history mechanism. Terminal Psi
retains the structural place, its callable-entry revision, and the current
place separately. It is the sole source pre-state term former, is admitted only
in fact position where a callable-entry revision exists, and packages cannot
implement or override it. The retired proof spelling `entry(place)` and the
retired explicit machine-member `entry` grammar are not aliases.

## Helper Machines

Large proofs should be decomposed through helper machines with small contracts.

```omega
machine Sort::compare_swap(
    before: &[Nat],
    items: &mut [Nat],
    index: u64
)
requires
    index + 1 < items.len
    Bag(items) == Bag(before)
ensures
    items[index] <= items[index + 1]
    Bag(items) == Bag(before)
{
}
```

The preservation fact is explicit. If a caller needs a before-state, it passes
one in. Nothing in this chapter relies on an implicit snapshot keyword.

A sorting proof is built from smaller facts:

```text
compare/swap orders one adjacent pair
compare/swap preserves Bag(items)
one pass moves the largest remaining item to the end
repeated passes establish Seq(items) in Sorted
Bag(items) stays equal to the explicit before value
```

## Quantified Facts

Universal claims at the outer level of a theorem use parameters checked
symbolically. That does not express every nested universal/existential claim.
General contracts must also quantify over arbitrary mathematical functions and
predicates. Their source syntax is not yet specified; the present parser and
bounded automation do not define the long-term limit of the proof language.

Constructive existence may supply a witness and law bundle. Nonconstructive
existence under explicitly admitted axioms need not produce an executable value.
Proof-local reasoning and runtime extraction are separate judgments.

The following is an executable-predicate example of bounded sequence reasoning,
not a recipe that replaces all quantified mathematics.

A relational property is defined by an ordinary measured machine:

```omega
machine sorted(items: &[i32]) -> bool
terminates by items -> Slice::Length;
{
    transition items.len <= 1 {
        true  -> true
        false -> items[0] <= items[1] && sorted(items[1..])
    }
}

domain [i32]::SortedAscending
    requires sorted(self);
```

The definition also specifies the decider: a checked validator runs it (or a
loop the checker proves refines it), and the successful path uses `as` only
after the predicate is established.

Consuming the fact at an arbitrary index needs one **extraction lemma** per
predicate — an induction, written once by the predicate's author:

```omega
machine sorted_extracts(items: &[i32], i: u64, j: u64)
requires sorted(items) == true && i < j && j < items.len
ensures items[i] <= items[j]
terminates by i -> Nat::IncreasingTo(j);
{ ... }
```

After that, the engine holds the quantified fact-shape natively and every use
is mechanical, under two closed rules:

- **Instantiation** happens only at index atoms in scope at the obligation —
  deterministic, budgeted, never searched. A missing instance is a normal
  "cannot prove" naming the index it needed.
- **The delta rule**: extending a quantified fact by one element (a
  validator's loop step, a table's append) costs one definitional unfold.
  Loop invariants over sequences ride state arrival contracts (chapter 11).

Instances injected by the lemma are ordinary atom-facts, so the
difference-bound engine composes them — transitivity, everything-left-of-mid,
min-at-ends are downstream chains, not further lemmas.

## Induction Is Ranked Recursion

A proof-stratum machine recurses under the same rule as every machine: a
`terminates by` ranking, checked at every cycle (chapter 3). Read as a proof, the
machine *is* the induction: transition dispatch is the case analysis
(exhaustiveness enforced — no missed constructor), the measured cycle is the
appeal to the induction hypothesis, and a state's arrival contract (parameter
facts plus state `requires`, chapter 11) is the hypothesis itself, proven at
every in-edge. Nothing was added to the language to express induction; the
state machine was already its shape.

Every recursive edge whose contract is consumed is an induction edge. This
includes a resultless statement citation, an explicitly discarded call, and a
call nested in a value expression. The callee's `ensures` may enter the proof
context only after that exact direct or mutual-cycle edge proves a strict
decrease under the component's ranking. Consequently an unmeasured theorem
cannot cite itself at unchanged arguments and use its own conclusion to close
its goal.

Induction may also be indexed by a finite unsigned count while its theorem is
about proof-only data. On an arm guarded by `n > 0` (or `n >= 1`), a recursive
argument `n - 1` is the checked predecessor. The structural checker treats that
argument as an opaque index, imports the recursive contract there, and can then
unfold or cite `Nat` lemmas around the recursive result. This is a bridge at the
recursive edge, not an implicit conversion between `u64` and structural `Nat`.

## Termination Proofs

Termination is a proof over every cycle in the reachable machine/state graph,
not an `ensures` proposition evaluated after a return and not a reach-row
member.

```omega
machine walk(items: &[Nat])
terminates by items -> Slice::Length;
{
}
```

The ranking argument is ordinary proof vocabulary:

- choose explicit subjects;
- select a well-founded ranking view; and
- prove every cyclic edge makes the produced rank strictly smaller.

Direction belongs to the view rather than a blessed `decreases` or `increases`
keyword. `Nat::Descending`, `Nat::IncreasingTo(limit)`,
`Tree::ProperSubtree`, and lexicographic views all satisfy the same checker
role. A standalone `measure` declaration supplies a named custom view and
multiple measures per carrier are legal.

Proof-stratum machines use exactly the same `terminates by` source and checking
rule as runtime machines. Their eligibility differs only at lowering: measured
non-tail recursion is legal when evaluation remains in the proof/compile-time
stratum and is rejected if runtime lowering is requested.

The normalized artifact separates the public termination guarantee from the
private ranking witness. A witness change invalidates its provider proof cache,
not caller or external requirement-binding identity. See chapter 9 and
[Termination, Ranking, And Progress](../pre_migration/design_briefs/termination_ranking_and_progress.md).

## Citing Proofs

A fact the engine cannot derive may be discharged by citing a proof machine's
contract, instantiated at the operands. This is the only connection between
proof-stratum theorems and runtime code, and it has no syntax of its own — a
cited theorem is a fact like any other:

```omega
machine Walker::step(&mut self)
requires self.n >= 1 && self.n <= 6148914691236517205
ensures self.n == collatz_step(n0)    // refinement: the u64 op IS the ideal op
{ ... }
```

Working rules:

- A theorem over parameters applies at any operands satisfying its
  `requires` — instantiation is machine application, not search.
- An `ensures` may equate a runtime place with a pure machine's result (a
  *refinement* fact): the runtime operation provably computes the
  mathematical function on the domain where its witnesses fit. Prove once
  over the ideal type; embed per width by supplying each width's bound.
- Runtime code that cites no proofs pays nothing and sees nothing.

Carrying a theorem to a site is an ordinary statement call — a fact-only
machine invoked for its `ensures`, which enters the flow facts and erases at
codegen:

```omega
mask_is_mod(self.head, self.cap);            // erased; its ensures now in scope
self.slots[self.head & (self.cap - 1)] = x;  // proves against those facts
```

Erasure does not remove the citation edge from recursion checking. A citation
inside the same recursive component imports an induction hypothesis only with
the strict-decrease certificate described above; syntax position and result
use do not weaken that rule.

This explicit form is the default: the proof structure
stays visible in the text. When an obligation fails for want of a known
lemma, the diagnostic names it by shape match. A rewrite extension —
proven equations joining the engine's term reading — is parked in the
design brief, to be revisited only if ergonomics demand it.

## Evidence And Trust

Facts are proven, computed, deferred, or accepted — and each tier is a
distinct compiler behavior, never a label:

- **Proven** (the engine, a derivation, a cited theorem): no declaration
  exists. Most facts live here invisibly.
- **Evaluated**: the compiler runs an ordinarily terminating machine in the
  hermetic target-semantic evaluator. Deterministic work is metered for live
  progress, warnings, and any root-selected ceiling; long or unlimited
  evaluation remains legal when root policy permits it. Results and canonical
  usage records are cached separately.
- **Deferred** ("prove later", written by tooling): a waiver of exactly one
  compiler-derived obligation — nothing new becomes citable. Warns on every
  build; fatal at **package release** (publishing with an open deferral is
  the hard error — "release" is a package-manager moment, not a build
  configuration; debt never crosses a package boundary). Hash-pinned to the
  code under it: edits kill the deferral and it must be re-taken.
- **Accepted**: a `boundary machine` — a contract with no body, the proof
  system's face of the boundary culture (chapter 19): explicitly trusted and
  reported, but not thereby proven or audited.

```omega
boundary machine collatz_cert_checked()
ensures check_collatz_cert(cert_blob_b41c) == true
```

Working rules:

- **The statement carries all specificity.** Trust the narrowest thing — an
  execution claim ("this checker accepts this certificate", the
  certificate's identity inside the statement) rather than the theorem it
  implies; a userspace proof machine lifts the narrow claim to the broad
  one. The trust report cannot be vaguer than the claim, because it *is*
  the claim.
- **There is no inline `assume`.** Boundary machines are the only home for
  unproven facts. Grant locality: **own-package boundary machines are active
  in dev builds**, carrying a standing warning until granted; boundary
  machines arriving **from packages are inert until granted** — a library's
  boundary machines surface as requests when the package is added, and a
  package can never self-grant.
- **Grants flow from the root.** `build.omg` declares dependencies and build
  selections through ordinary APIs. Install/update derives the complete
  package-qualified claim set from compiler checks and presents decisions to
  the consuming project. `omega.lock` records exact source pins, the complete
  normalized accepted baseline, and those decisions. Adding, removing, or
  changing a claim requires a decision on the exact diff; a package cannot
  accept its own claims for its consumers. The project trusts whoever lands
  these records, not a fingerprint or a supposed certificate of acceptance.
  A provider-slot grant binds only the provider plan selected for that slot;
  unselected and partial candidates remain dev-active and cannot acquire the
  selected plan's receipt merely because they implement the same boundary.
- **Acceptance is not proof of review.** A lock resolution, reviewer string,
  signature, LLM verdict, or proof certificate cannot establish that the
  package as a whole was competently audited. Certificates establish only the
  exact propositions independently reconstructed and checked by their kernel.
  The accepted project state and its surrounding organizational controls are
  the authority for package admission.
- **The engine can veto.** A boundary statement the engine can refute — one
  contradicting declared ranges, domains, or another accepted statement —
  is a compile error, grants notwithstanding.
- **Blast radius is reported.** The trust report names which conclusions
  rest on which boundary machines; facts derived without touching one stay
  in the unconditional tier, visibly. Export status is irrelevant — the
  report sees every grant, private or public. Routed provider qualifications
  remain equally specific: their rows bind the exact provider-plan
  fingerprint and requirement to the accepted parameter or returned result,
  authority flow, domain, carry policy, predicate-discharge requirement, and
  grant provenance.
- **The grant row is the language's `unsafe`.** A granted false statement
  can corrupt anything proofs protect — bounds, domains, and through
  corrupted memory, everything downstream. Reach restrictions cannot be waived by
  facts (they ride the call graph, and a boundary machine has no body),
  but a false range fact reaches the same place dynamically. Omega has no
  `unsafe` keyword because this is the one unsafe door: root-only, pinned,
  reported, tripwired.
- **Runtime-decidable boundary claims get oracle tripwires** in proof
  builds: a test run that witnesses a violation traps naming the machine
  that lied.

Certificates need no construct of their own: a certificate is wire data,
its checker is a measured machine, its soundness is a theorem
(`check(c) == true` implies the claim), and establishment is the
`evaluated` tier — or a proved `as` qualification through a certificate domain
(`domain [u8]::ValidCert requires check(self);`), the validated-decode pattern
of chapter 8 applied to proofs. A build that can afford the check *proves*
the claim outright; one that cannot accepts the narrow execution claim
above and lifts it by theorem.

Trust has a data face too. `boundary data` declares a type whose source
representation is externally admitted rather than structurally defined. It
does not mean “imported layout” or “exported layout,” and the keyword does not
encode traffic direction. A `boundary machine` is likewise classified by its
supply mode—external realization through `satisfies`, or an admission-bearing
claim—rather than by an inbound/outbound reading of `boundary`. An abstract
carrier-owned provider slot instead uses explicit `boundary requirement`.

The N5 `omega::language::core::real` package is temporary axiomatic scaffolding;
it is not precedent for a claim-free bodyless boundary-machine category. Its
current contents include:

```omega
boundary data Real;                                    // opaque proof-only carrier
boundary machine Real::add(a: Real, b: Real) -> Real;  // no ensures: a symbol — claims nothing
boundary machine real_add_commutative(a: Real, b: Real)
ensures Real::add(a, b) == Real::add(b, a);            // an axiom: one trust row
```

The carrier is proof-only. The claim-free `Real::add` spelling merely introduces
a temporary proof symbol; it must not mint a general language surface. N6/N8
replace it with the constructed Cauchy quotient and ordinary checked
operations. Each bodyless law carrying `ensures` is an admission-bearing axiom,
not a proved theorem, and remains one disclosed trust row until an ordinary
proof-machine body replaces it. Consumers then swap admission for checked
import.

Classical principles such as excluded middle are selectable assumptions, not
automatic truths supplied by a compiler or imported package. The complete
transitive assumption closure must be available to the consumer's policy.
Absence of one named axiom alone does not certify constructivity: the underlying
calculus and all other assumptions matter too. General axiom selection and its
proof-library surface remain migration work, not an implemented-core claim.

## Automation And Boundary

Omega source normally shows the proof strategy rather than every logical rule.
That is especially important when reading recursive proof machines. An
ordinary call contributes its checked `ensures` through contract application.
A call within the same recursive proof component is different: using that
contract would be circular unless the selected ranking proves the call strictly
smaller. Once that edge is checked under a well-founded relation, the callee's
instantiated `ensures` is the inductive hypothesis. `terminates by` therefore
does more than promise that computation stops; in a recursive proof it licenses
the logical assumption that makes induction sound.

The readable body is not the complete derivation. Elaboration records the
implicit computation, constructor rules, branch facts, inductive applications,
and licensed normalization in a kernel-checkable certificate. A deterministic
review synopsis is rendered from that certificate and names its fingerprint,
implicit closure rules, exact cited laws, and trust closure. It is never rebuilt
by a second analysis of the source: a plausible explanation of a different
proof would be worse than no explanation.

Automation does not erase authority or provenance. A normalization step names
the selected conformance and exact laws it consumes. If any cited law or
well-foundedness theorem is admitted, every dependent conclusion remains
admission-dependent. Total procedures may be replayed during checking; partial
search may not be trusted merely because it found an answer and must emit
checkable evidence.

The checker should automatically solve common cases:

- arithmetic normalization,
- equality reflexivity,
- range implications,
- branch facts,
- disjoint field facts,
- simple generic const facts.

When automation fails, library authors can provide helper machines. When a fact
cannot be proven from machine code, contracts, or boundary foundations, it must
cross an explicit boundary.
