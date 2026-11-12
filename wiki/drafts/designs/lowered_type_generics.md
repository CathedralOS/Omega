# Type and value parameters on the lowered form

Design proposal, 2026-11-11. Nothing here is built or run, and no fixture
outcome is claimed. It answers the part of condition 3 of
[the gate](check_on_lowered_form.md#the-gate) that
[its first result](check_on_lowered_form.md#condition-3-first-result) left open:
a generic body over a type parameter `T` cannot be checked once on the lowered
form today, because every value there has one concrete type. This record
proposes how the lowered form can carry a value whose type is still `T`, how
the body's obligations are stated against what `T` admits, and how each
specialization has the kernel re-decide the generic proofs without searching.
It recommends that, and recommends against checking only the specializations,
which would leave generic bodies unchecked at the alternatives nobody uses.

`#2378` checked a body over an implementation parameter
(`advance<Selected satisfies Step>`) once and carried its certificates into two
specializations. It found three things the lowered form cannot hold for a type
parameter
([lowered-discharge README](../../../omega-rust/psi/compilation/pipeline/lowered-discharge/README.md#what-the-lowered-form-does-not-carry)):

- a value row holds a concrete `ScalarType` (`Boolean`, `Integer(sign,
  width)`, `IeeeFloat(format)`), and a place a declared `StructuralTypeId`;
- terminal operations are selected per type (`ExactIntegerAdd` holds its
  `IntegerType`), and each safety goal comes from that type's range;
- `PropositionContext` maps each value to one `ScalarType`, and the integer
  rules depend on the width.

The proposal keeps all three for concrete values and adds one thing beside
each, used only by the generic check form, which is never encoded to
`PSITERM\0` bytes (as the [open question](check_on_lowered_form.md#open-questions)
on generic bodies already plans).

**Status, 2026-11-12.** Step 1 of [the order](#recommendation-and-order),
value parameters, is built and recorded as
[condition 3's second result](check_on_lowered_form.md#condition-3-value-parameters).
It differs from the text below in three ways. The literal a specialization
binds is worked out from the two lowered contracts, not taken from
`reverify_at_call`'s formal-for-actual substitution, because the typed trees
record a generated machine's argument only as its spelling. A requirement
with value parameters is read at the literals of each call, so the fail
twin is refused at `Units == 7`, where the member's refinement proof is
missing, not "for every `Units`, before any specialization". And a body that
reads the parameter as an operand is not carried yet.

Step 2, an opaque type parameter, is built and recorded as
[condition 3's third result](check_on_lowered_form.md#condition-3-type-parameters).
It differs from the text below in four ways. The sort is on the check
tables' value row, not beside `ScalarType`, which is matched at 263 places
across the compiler; the lowered form declares a rigid value with a stand-in
type that nothing may read. No kernel rule has an "any sort" premise yet:
no proposition may name a rigid value at all, which the checker enforces,
and the premise arrives with the first rule that needs it. There is no
rigid place type, so a `T` instantiated at a data type takes no proof, and
no abstract cleanup step. And a `T` value may be used once unless `T` is
`[copy]`, a restriction the text does not have, because the route has no
judgment of moves.

Step 3, a finite family, is built for generated machines only. The method
below noted that "whether stage 04 folds a type-equation branch in a
generated specialization was not checked"; it does not. The machine
generated for `u32` keeps `u32 == u32`, both arms and the arm of the other
type. The reader now decides closed type equations statically, so each
generated machine is read under its own type and judged as a closed machine:
`threshold<u32>` and `threshold<u64>` in
`pass/contracts/generic_effective_result_comparison_exit`, and `choose<u32>`
and `choose<u64>` in `fail/contracts/specialized_normal_call_wrong_type_contract`,
are each accepted by the route (compiler test `lowered_route_generics`).
That is the per-type check of [the middle form](#the-alternative-check-only-specializations),
for the types in use, and it is what a carried certificate would be decided
against. The generic form of such a body (rigid outside its arms, concrete
inside each) is not built: it needs a check-only terminator the lowered
form does not have, and whether it is needed at all is the question filed
as `finite-family-generic-check`. `core::integer::min` waits for
`contract-conjunct-under-type-equation`.

## What the specification asks

These sentences bind the design; each choice below cites the one it rests on.

- **S1.** "An unspecialized body checks every admitted alternative; one
  consumer cannot authorize an otherwise invalid generic body. Type parameters
  remain static even when their admissible set has several alternatives."
  ([generics.md, static type equality](../../spec/language/generics.md#static-type-equality))
- **S2.** "a branch with established `T == u32` may use that equality to
  check its operations and values as u32. On leaving the branch, only facts
  common to its surviving predecessors remain." (same section)
- **S3.** "Generic code proves declared preconditions and may use declared
  guarantees." and "A required member operation belongs to a named trait and
  is supplied through an explicit implementation parameter"
  ([requirements and conformance evidence](../../spec/language/generics.md#requirements-and-conformance-evidence))
- **S4.** "every use of `T` receives the property fixed by `T`'s declaration
  and concrete generic application"
  ([ownership, property declarations](../../spec/language/ownership.md#property-declarations))
- **S5.** "Carry checked derivations through specialization and published
  artifacts without re-searching under a consumer's smaller discovery budget."
  ([closed applications](../../spec/language/generics.md#closed-applications-and-canonical-identity)),
  read with decision `specialization-certificate-reuse`: every consumer
  re-checks a carried proof against its own goal.
- **S6.** A check's closure "starts from every declaration of every subject,
  including unused ones"
  ([validation subjects](../../spec/build/validation_subjects.md)).
- **S7.** `const Count: u32` binds a value that "must be statically known
  within the compiled specialization"; a runtime-capable binder "may lower to
  one body taking Count as an ordinary argument"
  ([value binders](../../spec/language/generics.md#value-binders-and-const-requirements)).
- **S8.** "Type-generic callable families similarly check every admitted type
  alternative. Consumers prove each demanded application satisfies the
  declared family rather than inferring an interface from their current
  instantiations." ([invocation-lifetime families](../../spec/language/generics.md#invocation-lifetime-families))
  and, for suspension carry, "Derive this condition during the single
  parametric body check"
  ([effects](../../spec/language/effects.md#conditional-suspension-carry)).

S1, S6 and S8 together say a generic body is judged as a declaration, for
everything it admits, and not through whichever applications exist. That is
the constraint the alternative at the end fails.

## Three kinds of parameter

A generic machine's parameters fall into three cases with different answers.

| Parameter | What its admitted set is | What the generic form carries | Witness |
| --- | --- | --- | --- |
| Value, `const N: u64` | every value of the carrier the `requires` admit | an ordinary value row of the carrier's type with no definition | `pass/facts/lexical_generic_bound_through_local_exit` |
| Opaque type, `T` with no type equation | every type with `T`'s declared properties | a rigid sort for each value of type `T` | `pass/facts/generic_comparison_body_through_match_join_exit` |
| Finite type family, `requires T == u32 \|\| T == u64` | the listed closed types | rigid outside type-equation branches; concrete inside each arm | `pass/contracts/generic_effective_result_comparison_exit`, `core::integer::min` |

### Value parameters

A `const N: u64` is a value of a known carrier, so the obstacle the README
names does not arise: it is a formal of type `u64` that no caller passes, and
its range comes from the generic's `requires`. In
`lexical_generic_bound_through_local_exit`, `Selected::value<const Units:
u64>(input)` requires `input <= Units` and ensures `result <= Units`; its body
returns `input`. The generic proof cites the `requires` row and the result
equation, both over `u64` values, so the width rules that apply are the same
in every specialization.

Specializing `Units := 7` is the substitution `reverify_at_call` already does
for actuals and formals: the generic value `Units` corresponds to the
specialization's literal value `7`, and every row the generic proof cites
still holds there. The specialization has one more hypothesis (`Units == 7`)
than the generic form, which a carried proof does not need. The fail twin,
`fail/facts/lexical_generic_bound_through_local_rejected`, differs only in
the trait member's requirement (`requires true`): the implementation then asks
more (`input <= Units`) than the member it stands for, so its refinement
witness fails for every `Units`, before any specialization. The same fixture
also passes the member through an implementation parameter, so it exercises
`#2378`'s carrier and the value substitution together.

A runtime-capable binder (`Count: u32`, no `const`) needs no specialization at
all under S7: its generic form is the one body.

Not covered: a value parameter in a type, such as `bytes: &[u8; N]` in
`pass/facts/borrowed_store_call_through_local_exit`. That needs a place type
whose extent is a value rather than a closed length, and the lowered route has
no arrays or references yet. It is the same addition as below for places of
type `T`, and it waits for the array family.

### Opaque type parameters: a rigid sort

Under S3 and S4, a body over an opaque `T` can do only what `T`'s properties
and its implementation parameters allow: move it, copy it if `T [copy]`, clean
it up, store it in a place of type `T`, pass it to a call, and call members of
an implementation parameter over `T`. No arithmetic, comparison or layout
operation selects on `T`.

The generic form carries:

- **A value sort.** Beside `ScalarType`, a value row may hold
  `Rigid(TypeParameterId)`, the dense identity of the binder in the
  declaration. `PropositionContext` maps a value to a sort, not only to a
  `ScalarType`. A rigid value has no range, no width and no literal.
- **A place type.** `StructuralTypeId` gains a rigid entry for `T`, and an
  open data application (`Pair<T>`) is interned by its declaration and its
  argument tuple of sorts. A field of type `T` is rigid; other fields keep
  their concrete types. A rigid place has fields only through such an
  application, never by itself. (Generic data is refused today as `type
  generic data`; this is the shape it would take.)
- **Parametric operations.** The operations that only move a value (move,
  copy, store, load, pass as an argument) take the sort where they take a
  type. Cleanup of an affine rigid
  value is an abstract cleanup step whose contract is the type's published
  cleanup, discharged in the specialization by the concrete route. No
  arithmetic operation takes a rigid sort, so the per-type selection the
  README describes is untouched.

**Obligations against `T`'s contract.** Every obligation the generic form owes
is one the existing rows already state: a call's `requires`, a guarantee, a
cleanup, a loan. What is new is only that some of the values they mention are
rigid. In `generic_comparison_body_through_match_join_exit`, `same<T>(left:
T, right: T)` returns `false` and `compare<T>` returns `same<T>(left, right)`;
the generic proof that `compare` returns `false` mentions two rigid values only
as call arguments. In `abstract_signature_call_generic_actual_through_local_exit`,
the requirement `premise<T>(kept)` is a proposition over a call term whose
argument is rigid. Neither proof reads `T`.

**Why one check covers every alternative (S1).** Each kernel rule declares
what it needs to know about the sort of each value it mentions. Equality,
transport, Boolean rules, contract citation and call congruence need nothing;
integer bound rules need a width. A rule instance over a rigid value is
admitted only if its sort premise is "any sort". A certificate the kernel
accepts in the generic form therefore uses only rules whose conclusion holds
at every substitution of `T`, which is the admitted set S1 asks for, and it
cannot use a fact that only one type would make true. This is a property of
the rule table that a test can enumerate; it is not trusted at carry time,
because the kernel re-decides the substituted certificate anyway.

The fail twin `fail/facts/generic_comparison_body_through_match_join_rejected`
differs only in `same` returning `true`; what the caller learns from
`compare<u64>` is then `true`, and `hold`'s `requires direct == false` is
refused. That knowledge crosses from the body, which the record allows inside
one fingerprinted verification unit; it is stated over a Boolean result and
carries unchanged.

### Finite type families: arms under their equation

S2 lets a static branch on `T == u32` check its operations as `u32`.
`threshold<T>()` in `generic_effective_result_comparison_exit` returns `37u32`
in the `T == u32` arm and `73u64` in the other, which `requires T == u32 ||
T == u64` leaves as `T == u64`. The generic form lowers:

- the part of the body outside type-equation branches with `T` rigid, as
  above;
- each arm with `T` replaced by that arm's type, as an ordinary concrete
  region whose operations are selected for that type. Its proofs are ordinary
  concrete proofs.

A type-equation branch is a check-form terminator (`TypeEquationBranch`),
never encoded. The arm's equation is static context for typing, not a
knowledge row, which keeps the record's rule that facts never feed operator
selection: S2 makes the equation a static proposition, and the arm is typed
under it. The `_` arm's equation comes from the `requires` disjunction minus
the arms before it; an arm with no remaining alternative is unreachable and
owes nothing.

This checks every admitted alternative (S1) while lowering each arm once, not
the whole body once per alternative. `core::integer::min<T>` has eight
alternatives and eight one-literal arms, so its generic form is eight small
concrete regions and one rigid result.

**Contracts with type equations.** `min<T>` ensures `(T == u8 && result ==
0u8) || ... || (T == i64 && result == ...)`, with `result` of sort `T`. Two
kernel additions serve it:

- a cast rule: under a hypothesis or conjunct `T == u8`, a value of sort
  `Rigid(T)` may be read at sort `u8`. This is S2 applied inside a
  proposition;
- closed type equations decide by canonical type identity, from the one
  normalizer that owns it ([static type equality](../../spec/language/generics.md#static-type-equality)).

Whether a conjunct of a contract is typed under the equation beside it, as
`min`'s `ensures` needs, is not stated for contracts (question 2 below).

## What a carried certificate looks like when the type changes

`#2378`'s carrier checks that the generic and specialized machines have the
same values and calls one for one, and refuses otherwise. With a type
substitution `σ` (`T := u32`) the check becomes: same values and calls after
applying `σ` to every sort, and after removing the arms whose equation `σ`
makes false. A specialization that stage 04 generates with the branch already
folded has exactly the selected arm's blocks; one that keeps the branch has
the dead arms too, and the carrier ignores them.

The carried certificate is the generic one with three mechanical rewrites,
each linear in its size:

1. **Sorts.** Every `Rigid(T)` in its context becomes `σ(T)`. By the rule
   discipline above, no step changes meaning.
2. **Type equations.** Every closed equation `σ(T) == u32` folds to `Truth`
   or `False`. A disjunction introduction whose other disjuncts fold to
   `False` is kept; the consumer's goal, which it computes from its own
   module with the same fold, is then the surviving disjunct, and the carrier
   offers the sub-certificate for it. Casts under a folded equation become
   identities.
3. **Arms.** Certificates from the arms `σ` selects are taken as they are;
   they were concrete already. Certificates from the other arms are dropped:
   they discharged other alternatives in the generic check.

Citations of the requirement's guarantee are replaced by the implementation's
refinement witness, exactly as `#2378` does. The kernel decides the result
through `verify` against goals and hypotheses the specialization computes;
nothing searches and nothing about the carried certificate is trusted. If a
rewrite yields a certificate the kernel refuses, the specialization is refused
and no search is started, as `#2378`'s
`a_body_that_only_holds_for_the_implementation_in_use_is_rejected` shows for
implementation parameters.

## What it costs

Unmeasured; these are counts of what must be added, not lines.

- **Lowered form:** one sort variant on value rows and in
  `PropositionContext`; one rigid entry and open applications in the
  structural type table; parametric operations taking a sort; an abstract
  cleanup step; one check-only terminator. No new obligation kind,
  justification or callee variant, which matches `#2378`.
- **Kernel:** a sort premise on each rule, defaulting to "concrete" so that
  an unmarked rule refuses rigid values; one cast rule; closed-equation
  decision through the existing normalizer.
- **Carrier:** the substitution, the correspondence check modulo dead arms,
  and the fold. All linear, all without the prover.
- **Check time:** the generic body once, each arm once, and per
  specialization only the kernel. Against checking every specialization by
  search this saves the search for each consumer; no timing exists for either
  (`#2378` took none).
- **Proof power:** a generic proof cannot use width rules on rigid values. A
  body that needs them must branch on a type equation, which S2 already
  requires of the source.

## The alternative: check only specializations

Skip the generic form. Stage 04 already generates one closed machine per
static application; check each one concretely by search, as the route does for
any machine. Nothing new is carried on the lowered form, certificates stay
concrete, and every type the route handles works at once.

It is not what the specification asks:

- **S1 and S8.** A body invalid at an undemanded alternative is accepted.
  With `core` as a check subject, `core::integer::min` with a wrong `i64`
  literal would pass whenever nothing in the check calls `min<i64>`, which is
  the demanded applications authorizing the body. (As a dependency nothing
  demands, `min` is correctly not checked at all, under S6.)
  `implementation_parameter_contract_only_holds_for_selection` is this case
  for implementation parameters: a search over each of its two
  specializations proves its bound, and only the generic check refuses it.
  There is no type-parameter fixture yet where the two choices disagree; one
  pair (a finite family whose undemanded arm breaks its `ensures`) would pin
  it, within the per-PR file cap.
- **S6.** A generic subject declaration with no application is never checked,
  so condition 1's "an unused subject declaration with an invalid body is
  rejected" fails for generics.
- **S5.** Each consumer searches again under its own budget, which is what
  "without re-searching under a consumer's smaller discovery budget" rules
  out.

Consequence for the gate: condition 3 cannot pass as written ("checked once as
written, against its declared requirements and with its parameters left
abstract"), and condition 1 would need a generics carve-out. Both are the
owner's to rewrite; this record does not.

A middle form is possible for finite families only: check each declared
alternative as a concrete specialization, independent of consumers. That meets
S1 for the listed types at the cost of the whole body per alternative (eight
for `core::integer`), cannot serve an opaque `T`, and whether it counts as
"checked once" is the owner's reading (question 3).

## Questions for OWNER_QUESTIONS.md

Not decided here. Each is a language or gate question; the lowered-form
representation itself is delegated
([OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md), Psi representation
paragraph). They are filed there, in this order, as the decisions
`type-family-operation-outside-arm`, `contract-conjunct-under-type-equation`
and `finite-family-generic-check`, each with what the current checker does
with a small program of that shape.

1. **Operations on `T` outside an arm.** In a finite family, may a body apply
   an operator to a value of type `T` outside a branch that establishes one
   equation (`a + b` under `requires T == u32 || T == u64`)? S2 grants typing
   inside a branch only. If it is allowed, checking every alternative needs a
   per-alternative operation in the generic form, which this design does not
   have; no corpus program was found doing it.
2. **Contract conjuncts under a type equation.** Is `result == 0u8` in `(T ==
   u8 && result == 0u8)` typed under the equation beside it, and in `min<u32>`
   is that disjunct vacuous or ill-typed? `core::integer` and
   `pass/core/integer_query_public_refinement_exit` rely on the first reading;
   `data_and_literals.md` uses the same shape in a data `requires`
   (`Command<T>`). No sentence states it for `ensures`.
3. **Condition 3 for a finite family.** Does checking each declared
   alternative concretely, without a rigid form, meet "checked once as
   written ... with its parameters left abstract"?

## Recommendation and order

Carry type parameters on the lowered form: value parameters as undefined
formals, opaque types as a rigid sort with sort-free rules, finite families as
rigid outside and concrete inside each arm; carry certificates by sort
substitution, equation folding and arm selection, re-decided by the kernel.
Do not check only specializations: it fails S1, S5 and S6 and makes condition
3 unpassable as written.

Order, smallest change first, each with its pass and fail twin and `omega run
--both`:

1. `const Units: u64` in `lexical_generic_bound_through_local_exit`: no new
   carrier, only substitution of a constant for a formal.
2. Opaque `T` in `generic_comparison_body_through_match_join_exit`: the sort
   and the rule premise.
3. Finite family in `generic_effective_result_comparison_exit` and
   `specialized_normal_call_wrong_type_contract`: arms, the fold, and then
   `core::integer::min` once question 2 is answered.

## Method

Read on `main` at `a9779875f`: `check_on_lowered_form.md` (the gate and its
open questions), the `#2378` description, the lowered-discharge and
structure-elaboration READMEs, `ScalarType` (`semantic-vocabulary/src/scalar_terms.rs`)
and `PropositionContext` (`semantic-vocabulary/src/proposition.rs`), the
cited spec sections, and the witness fixtures and their rows in
`tests/omega/corpus_outcomes.txt`, where each is `checked` (pass) or
`rejected expected` (fail) on the current pipeline. The lowered route was not
run on any of them; whether stage 04 folds a type-equation branch in a
generated specialization was not checked.
