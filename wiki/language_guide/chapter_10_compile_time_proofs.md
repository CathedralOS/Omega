# Chapter 10: Compile-Time Proofs

Compile-time proofs use ordinary machines and contracts. The basic judgment is:

```text
requires + body facts -> ensures
```

The checker checks a body under its declared assumptions. A caller must establish
those assumptions for the actual arguments before using its conclusions. An
unproved conclusion rejects unless it is an explicitly accepted boundary claim.
Failure to prove a statement is not proof of its negation.

This chapter teaches the intended model. General mathematical source forms and
foundations remain [undetermined](../spec/proofs/contracts.md#undetermined-foundations);
the current [source automation](../../omega-rust/psi/semantics/validation/README.md#source-proof-automation)
implements a bounded fragment. Schematic examples below do not claim complete
compiler support or supply omitted proofs.

## Contracts and evidence bundles

A contract states a formula; a checked proof establishes it. A mathematical
condition need not have an executable decision procedure. For a decidable
condition, write its Boolean truth explicitly, such as `sorted(items) == true`.
Checking a contract is not a hidden runtime validator call.

Ordinary traits group operations, witnesses, and their laws. A named conformance
supplies the complete bundle; its name or shape proves nothing by itself. For
example, a convergence bundle can group a modulus with a theorem about sequence
values after that modulus. Composing two such bundles must prove the composed
modulus's law, not just construct another value called `ConvergenceEvidence`.

General quantification must also support arbitrary mathematical functions and
predicates, not only executable declarations. Neither a dedicated proposition
declaration nor a machine returning a logical result is a prerequisite. Optional
[formula naming syntax](../proposals/proof_formula_syntax.md) is a separate,
unproven ergonomic proposal, not accepted source syntax.

The [proof contract](../spec/proofs/contracts.md#machines-and-bundles) owns these
rules and their foundational requirements.

## Evidence identity and validity

Keep the statement, selected witnesses, and derivation provenance distinct.
Two proofs of the same statement do not make different witness functions equal.
Repeated projection of one bundle retains its witness; forwarding and call
substitution must preserve that relationship.

A conclusion applies only on the paths and result cases for which it was proved.
Facts about borrowed or revisioned data expire with their relevant scope or an
invalidating write. Bundling does not make a conditional fact unconditional or
hide its transitive assumptions. Logical facts cannot mint or consume authority;
authority remains in independently tracked Type occurrences.

[Identity and availability](../spec/proofs/contracts.md#identity-availability-and-erasure)
defines the exact substitution, scope, publication, and replay obligations.

## Explicit relevance

Relevance belongs to a binding occurrence, independently of its type's
multiplicity and its validity scope. An erased field uses a binding property:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

Here `Valid<T>` stands for the evidence type supplied by the example's author.
The checker retains `proof`, but lowering gives it no field offset, address,
runtime read, or runtime cleanup. It may supply proof computation or statically
checked authorization; it may not determine runtime data or control. This rule
applies through calls and projections, not just direct field access.

Erasure does not discharge ownership. An erased Type witness can still be
affine, linear, loan-scoped, or provenance-bearing. Its obligations remain live
until discharged; a containing value cannot leave scope with an outstanding
erased linear obligation. No runtime destructor may depend on erased storage.
Logical proof facts, in contrast, are proof-only and copyable.

A zero-layout Type value is not automatically erased. Conversely, `[erased]`
is not permission to delete the representation of an ordinary runtime value.
Construction normally supplies the erased term. Omission is allowed only when
an accessible visible nullary constructor determines it structurally, not by
general inhabitance search, defaults, or inferred zero values.

Runtime layout, ABI, codecs, and placement skip erased fields; semantic identity
retains them. Placement must establish any required erased facts through its
checked or admitted plan rather than invent a physical offset. See
[explicit erased bindings](../spec/proofs/contracts.md#explicit-erased-bindings)
and [layout plans](../spec/layouts/plans.md).

## Machines As Proofs

This machine proves an ordering fact:

```omega
machine distinct_indices(i: u64, j: u64)
requires
    i < j
ensures
    i != j
{
}
```

Its empty body is valid only because the conclusion follows from the assumption
and the checked order rules. An empty body does not waive a proof obligation.

A closed arithmetic example is:

```omega
machine pythagorean_3_4_5()
ensures
    3nat * 3nat + 4nat * 4nat == 5nat * 5nat
{
}
```

The checker reduces both sides to the same natural value and closes equality
by reflexivity. A theorem-only machine has no Type result: parameters state its
subjects, `requires` its hypotheses, and `ensures` its conclusions. Return a
value only when the machine genuinely computes one as well as proving its
contract; law slots do not need dummy returned witnesses.

## Typed Facts

The operand types choose the mathematics. `3nat * 3nat` is unbounded natural
arithmetic; `3i32 * 3i32` carries the selected machine-arithmetic obligations.
The operator spelling alone does not select proof rules.

Eligible calls and projections in fact position are denotational terms.
For example, `add_int(a, b).pos` denotes a field of a pure total call result;
it creates no runtime temporary, move, or loan. Its validity nevertheless
depends on every referenced occurrence and revision. A fact depending on an
existing loan still expires with that loan.

A proposition can mention a linear value without acquiring its custody or
making that value copyable. Exact operation identity and proof eligibility
remain checked even when no runtime call is emitted.

## Total Specification Arithmetic

Contract terms, domain predicates, and guarded crash conditions must be total.
Exact arithmetic first proves representability. Wrapping and Saturating still
prove primitive definedness, such as a nonzero divisor. An operation that can
transfer runtime control does not become a proof term merely by appearing in a
contract. Comparisons on a Trapping-qualified value are a different operation
from trapping arithmetic.

Use an explicit proof view when a condition needs unbounded mathematics:

```omega
requires
    embed(left) + embed(right) <= embed(i32::Maximum)
```

Integer and address carriers embed into proof `Int`, retaining their exact
carrier bounds. The projection does not change qualification or produce runtime
data. Embedding an already wrapped sum preserves that sum's selected meaning;
it does not retroactively perform unbounded addition.

Removing a policy with `as` instead selects Exact carrier arithmetic:

```omega
requires
    embed(right) >= 0
    embed(left) <= embed(i32::Maximum) - embed(right)
ensures
    result == (left as i32) + (right as i32)
```

This contract fragment assumes `left`, `right`, and `result` have the appropriate
integer types. Earlier facts establish the result's representability. An
operation cannot use its own containing proposition to justify its formation.

Ordinary `Nat - Nat` likewise requires the right operand to be no greater than
the left. Clamping is explicit `Nat::saturating_sub(left, right)`. Conversion
from `Int` to `Nat` requires nonnegativity.

Floats use `Float::meaning32` or `Float::meaning64`, not integer embedding.
`FloatMeaning` preserves finite rational value, signed zero, infinity, and NaN.
Its structural proof equality is not IEEE equality: its NaN case is reflexive,
and its two zeros are distinct. See [total arithmetic](../spec/proofs/contracts.md#total-arithmetic),
[numeric values](../spec/language/numeric_values.md), and
[mathematical values](../spec/terminal-psi/mathematical_values.md).

## Proof-Only Data

Recursive mathematical data can express values without a finite runtime layout:

```omega
data Nat {
    case Zero;
    case Succ(n: Nat);
}
```

This illustrates the structural natural numbers; it is not an instruction to
redeclare core's type. Such values participate in proof computation, not
runtime-relevant storage. Explicit relevance and ordinary Type multiplicity
still apply; an all-fieldless sum such as `bool` does not become erased simply
because none of its cases has a payload.

Exact arithmetic over `Nat`, `Int`, and `Rat` can compute mathematical values.
Reasoning over an axiomatized carrier instead uses its selected laws and retains
their assumptions. A pure total machine over runtime types may also be used in
facts; ordinary arithmetic proofs do not all require unbounded data.

A quotient groups representatives related by a proved equivalence. Its logical
equality means the representatives belong to the same equivalence class, not
that their representations are equal. The intended Real construction provides
a schematic example:

```omega
data Real = CauchySeq % ConvergesTogether
where
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

The relation name here is mathematical schematic notation: general relation
source forms remain undetermined. `CauchyEquivalence` names an explicitly
selected conformance, not an ambient search for similarly named laws. The
conformance licenses formation; operations need their own selected congruence
proofs. Different sequence generators can be related within the same family
when the declared relation supports their different indices.

Construction may retain the supplied representative unchanged. It does not
license extraction, structural matching, hashing, serialization, or an executable
`==`. A lifted observer must establish its actual law: equality, for example,
must decide the equivalence relation, not merely return the same answer for
equivalent representatives. Public canonical bytes need additional evidence;
ordinary opaque constant materialization does not require canonicalization.

The [quotient specification](../spec/proofs/quotients.md) owns formation,
`Quotient::define` versus `Quotient::lift`, exact theorem roles, constructor
relations, observer laws, and current publication limits. Initially, effectful
operations and custody-bearing carriers are outside that gate: result congruence
alone cannot preserve effects or make distinct authority occurrences interchangeable.

Core's current Rat/Cauchy proofs and temporary axiomatic Real implementation are
described in the [mathematical library note](../../source/library/core/mathematics.md).
They do not establish that the full intended quotient or foundations are implemented.

## Proof Views

A proof view lets a contract describe runtime data mathematically without
allocating a runtime model. For a sorting specification, useful schematic views
are `Seq(items)` for order and `Bag(items)` for element multiplicity. These are
library proof concepts, not an invitation to add compiler-known sorting forms.

By contrast, `embed` and `old(place)` are compiler-owned fact-position term
formers; packages cannot override them. `old(place)` selects a structural place's
callable-entry revision. It does not copy an owned value or take a snapshot of an
arbitrary computed expression. The pre-state and current place retain their
ordinary revision and custody identities.

An explicit `before` parameter remains useful when the caller wants to supply
a separately obtained mathematical snapshot. `old` serves the narrower case
where the contract refers to the entry revision of an existing structural place.
See [source conservation contracts](../spec/resources/content_custody.md#source-conservation-contracts).

## Helper Machines

Break large proofs into helpers with small contracts. This schematic sorting
helper assumes the sequence/bag vocabulary and its supporting laws are in scope:

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
    ... // implementation and proof omitted
}
```

This is proof-side schematic data, not a runtime slice-layout claim for `Nat`.
The caller supplies `before` explicitly. A complete sorting proof then composes
adjacent ordering, bag preservation, the pass invariant, and the final sorted
sequence property. Each helper must prove its guarantee; the contract alone
does not establish it.

## Quantified Facts

The parameters of a theorem express an outer universal claim: its body is checked
for arbitrary admitted arguments. General mathematics also needs nested universal
and existential claims and quantification over arbitrary functions and predicates.
Those source forms remain design work, not an implicit limit imposed by today's
automation.

For a decidable property, an ordinary measured machine can compute a Boolean.
For example, a sequence-sortedness predicate recursively compares adjacent values.
A domain can require `sorted(self) == true`; a checked runtime validator must
establish that fact before qualification. This executable approach is useful
but does not replace general mathematical predicates or nonconstructive existence.

An extraction theorem can connect such a definition to arbitrary elements:

```omega
machine sorted_extracts(items: &[i32], i: u64, j: u64)
requires sorted(items) == true && i < j && j < items.len
ensures items[i] <= items[j]
terminates by i -> Nat::IncreasingTo(j);
{
    ... // induction and supporting definition omitted
}
```

After establishing the theorem, callers cite its contract at their exact indices.
The current prover contains narrow first-order sequence shapes, not unrestricted
quantifier search. A missing instance or exhausted automation yields an unproved
obligation, not a counterexample. Current automation belongs beside
[validation](../../omega-rust/psi/semantics/validation/README.md).

## Induction Is Ranked Recursion

A proof machine recurses under the ordinary `terminates by` rule. Transition
cases provide case analysis; the smaller recursive call provides the induction
hypothesis. State arrival contracts must hold at every incoming edge.

The recursive contract becomes available only after that exact direct or mutual
cycle edge proves strict decrease under a well-founded ranking. This applies
to statement citations, discarded results, and calls nested in expressions.
An unmeasured theorem cannot cite itself at unchanged arguments and then use
its own conclusion to close the goal.

Earlier citations may establish a later citation's assumptions, never the
reverse. A guarded finite unsigned predecessor may connect arithmetic descent
with a theorem about structural mathematical data, but it is not an implicit
conversion between an integer carrier and `Nat`.

## Termination Proofs

Termination concerns cycles, not an `ensures` proposition checked after return
or a member of service reach. A ranking chooses subjects and a well-founded view:

```omega
machine walk(items: &[Nat])
terminates by items -> Slice::Length;
{
    ... // traversal omitted
}
```

The ranking view supplies direction. Descending naturals, increasing-to-a-limit
views, proper subtrees, and lexicographic views all require checked decrease
at cyclic edges. A custom `measure` supplies a named view.

Proof/compile-time recursion may be non-tail when its ranking is proved.
Runtime recursion is tail-only and lowers to constant-stack iteration. The
public termination guarantee is distinct from the private ranking witness;
changing that witness does not rename the caller's requirement.
[Termination](../spec/language/termination.md) and
[citation and induction](../spec/proofs/contracts.md#citation-and-induction)
own the complete rules.

## Citing Proofs

An ordinary statement call carries a theorem to a use site. In this schematic
ring-buffer fragment, the named theorem's contract establishes the required
index bound under the caller's facts:

```omega
mask_is_mod(self.head, self.cap);
self.slots[self.head & (self.cap - 1)] = x;
```

The fact-only call erases at runtime. Its assumptions still need proof, and
its exact instantiated conclusions enter the flow facts. It does not enable
an ambient rewrite rule or evade recursive-edge checking.

A contract can also state refinement: a runtime operation equals a pure
mathematical operation under explicit bounds. Supply any needed pre-state
subject through an explicit parameter or the established `old(place)` form,
not an undeclared snapshot name.

## Evidence And Trust

Keep four ways of handling an obligation separate:

| Route | What it establishes |
| --- | --- |
| Checked proof | A derivation under its exact premises and assumptions. |
| Evaluation | A result from eligible terminating target-semantic computation. |
| Deferral | A site-pinned waiver of one obligation, not a reusable fact. |
| Admission | An explicitly trusted boundary claim, not a checked theorem. |

Evaluation is metered under root policy; long or unlimited evaluation can remain
legal when that policy permits it. Cached results and usage records are distinct.
A deferral warns on every build, is invalidated by relevant edits, and cannot
cross package release. See [evaluation](../spec/language/evaluation.md),
[logical work](../spec/resources/logical_work.md), and
[axioms and receiving policy](../spec/proofs/contracts.md#axioms-and-receiving-policy).

Trust the narrowest needed claim. This schematic boundary declaration admits
one certificate-checking result, assuming its exact checker and data are in scope:

```omega
boundary machine collatz_cert_checked()
ensures check_collatz_cert(cert_blob_b41c) == true;
```

An ordinary soundness theorem can lift that exact execution claim to the
mathematical result. A build that actually checks the certificate need not admit
the execution claim. Certificate data needs no special source construct.

Grants come from the consuming root, not the dependency making the request.
Acceptance binds the exact selected claim or provider plan and its source pins;
an unselected candidate cannot borrow a selected plan's receipt. A lock entry,
signature, reviewer label, or checked certificate does not prove that the entire
package was competently audited. Reports retain each dependent conclusion's
assumptions, including private claims and exact routed authority provenance.
See [package review](../spec/packages/review.md) and
[provider selection](../spec/build/provider_selection.md).

There is no inline `assume`. Own-package boundary machines are active in
development builds with a standing warning until granted; dependency boundary
machines are inert until granted. A package cannot grant its own claims on
behalf of its consumers. Development use is not consumer or release acceptance.

A boundary statement the checker can refute against declared ranges, domains,
or another accepted statement rejects despite a grant. This is not a guarantee
that the complete assumption set is consistent. Runtime-decidable boundary
claims require oracle tripwires that trap on witnessed violations and identify
the claim. Their activation, coverage, and failure-reporting protocol remains
undetermined, not a promise of currently implemented instrumentation. These
rules belong to [receiving policy](../spec/proofs/contracts.md#axioms-and-receiving-policy).

Admitting a false statement can invalidate downstream safety proofs. It does
not waive the independently checked reach ceiling or establish runtime authority
merely by asserting a fact. `boundary data` separately admits representation;
the word `boundary` does not encode inbound versus outbound traffic. See
[capabilities and boundaries](chapter_19_capabilities_effects_boundaries.md).

Classical principles are selectable assumptions, not compulsory truths imported
with a library. Consumers need the complete transitive assumption closure.
Absence of one named axiom alone does not certify constructivity: the calculus
and all other assumptions matter too. A checked quotient theorem can depend on
admitted premises without becoming an assumption-free guarantee.

## Automation And Boundary

Source normally shows proof strategy rather than every inference. Elaboration
must account for implicit computation, constructor rules, branch facts,
contract applications, and licensed normalization in independently checkable
evidence. A review synopsis is derived from that certificate and its attribution
metadata, not a second guess at the source's proof.

Normalization names the selected conformance and exact laws it consumes.
Inherited assumptions remain visible. A specified total procedure may be
replayed, but totality alone is not soundness. Partial or heuristic search
must supply checkable evidence rather than demand that a checker trust its
answer or repeat the search.

Automation should handle common arithmetic, reflexivity, ranges, branch facts,
and simple generic facts. When it cannot, authors supply explicit helper proofs
or a reviewed boundary claim. The distinction between intended certified
elaboration and current trusted source automation remains important; see
[certified elaboration](../spec/proofs/contracts.md#certified-elaboration-and-review)
and the [implementation note](../../omega-rust/psi/semantics/validation/README.md#source-proof-automation).
