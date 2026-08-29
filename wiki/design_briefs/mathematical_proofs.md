# Design Brief: Mathematical Proofs

Current as of 2026-08-28. Omega does not introduce a second proof language.
Proofs use ordinary machines, data, contracts, domains, and ranked recursion;
proof-only uses erase after checking.

The proof language has one dedicated formula declaration. A `proposition`
names a fact; ordinary proof machines establish it through `ensures` and use it
through `requires` or proof expressions. Primitive propositions end in `;`,
witness-bearing propositions publish one carrierless evidence interface as
fingerprinted proof content, and transparent proposition definitions use `=`.
The witness-bearing interface follows the proposition signature in an
`evidence Interface;` clause. See
[Law-Bearing Relations, Evidence, And Quotients](law_bearing_relations_and_quotients.md)
for the complete source and evidence model.

Ambient proposition terms are erased and copyable. A one-shot permission or
other consumable authority is an affine or linear Type carrier, possibly with
zero runtime layout, and follows ordinary ownership rather than creating a
second custody calculus for proofs. Resource-sensitive mathematics remains
expressible as an object logic over user-defined carriers, proposition
families, and entailment laws.

## Proof machines are ordinary machines

A machine used to establish facts is not a separate declaration species. Its
parameters state the universal variables, its `requires` state hypotheses, and
its `ensures` state the theorem.

```omega
machine add_zero_right(n: Nat) -> Nat
    terminates by n -> Nat::Descending;
    ensures result == n
{
    ...
}
```

The same checked machine contract may serve runtime, compile-time evaluation,
or proof citation when its types and reach permit it. A fact-only invocation
emits no runtime work.

A theorem-only machine has no `Type` result. Its parameters quantify the
theorem, its `requires` are hypotheses, and its `ensures` are conclusions. A
return type is reserved for a machine that genuinely computes an observed
value as well as proving a contract. Algebraic law slots and
quotient-congruence theorems are theorem-only; dummy `-> Self` results are
retired rather than treated as proof evidence.

## Quantification and proof data

- Universal claims use machine parameters checked symbolically.
- A generic accepted axiom over `<machine M>` spends one grant on the
  normalized template statement and its required machine contract. Instances
  record the selected machine-contract identity but do not spend another
  grant; narrowly trusted instances use non-generic accepted facts.
- Element-wise claims use element domains/types.
- Prefix/window claims use bounded views such as `items[0..loaded]`.
- Relational sequence claims use predicate machines plus extraction lemmas.
- Recursive mathematical structures such as `Nat`, `Seq<T>`, `Bag<T>`, and
  exact rational/integer forms are proof-only when they have no finite runtime
  representation.

Proof-only status is structural, not a `[proof]` or `[unbounded]` property.
Anonymous binder syntax remains optional sugar to consider only if named
predicates prove too verbose.

Runtime fixed-width integers and addresses enter unbounded proof arithmetic
through the total `embed(value) -> Int` projection, which also establishes the
source carrier's exact range. This is distinct from erasing an arithmetic
policy with `as`, which selects Exact carrier arithmetic and retains its
representability obligations. Floats use `FloatMeaning` so signed zero,
infinity, and NaN are not lost. Direct Trapping arithmetic never forms a proof
term; see
[Total Specification Arithmetic](total_specification_arithmetic.md).

## Ranked recursion

Every terminating recursive call cycle requires a `terminates by` ranking.

- Proof/compile-time recursion may use the ranked structure required by the
  theorem.
- Runtime recursion is legal only in tail position and lowers to constant-stack
  loop machinery.
- Runtime non-tail recursion is rejected; recursive depth belongs in explicitly
  sized data rather than hidden activation frames.
- Transition loop-backs are jumps, not call recursion, and may be productive
  indefinitely.
- Mutual recursion requires a joint well-founded ranking.

For the common bridge from finite runtime counts into structural mathematics,
a proof machine may rank on an unsigned integer parameter and recurse through
the guarded predecessor shape `n > 0` / `n - 1`. The arithmetic checker proves
that edge well-founded; the structural judge may instantiate the recursive
contract at the opaque predecessor term and reason about a surrounding
`Nat::Succ`. Neither checker fabricates facts in the other's domain.

This gives induction its natural source form without weakening the systems
language's stack guarantee.

## Explicit proof citation

A theorem reaches a proof site through an ordinary statement call:

```omega
add_comm(a, b);
```

The callee's already-checked `ensures` is instantiated at the call operands and
added to flow facts. The call erases when it is fact-only.

Omega does not activate global rewrite rules from imports. Diagnostics may
shape-match a failed obligation and suggest a useful lemma, but the source must
contain the citation. Citation cycles are ordinary machine-call cycles and
therefore obey ranked termination. Every recursive citation whose `ensures` is
consumed is an induction edge, even when the call is resultless, explicitly
discarded, or nested in another expression. Its contract enters the proof
context only after that exact edge proves a strict decrease under the direct or
mutual component's ranking.

## Algebraic canonicalization

Engine-internal normalization is distinct from lemma rewriting. A carrier earns
normalization through explicit conformance to an algebraic trait whose operation
and law requirements are proved.

`CommutativeSemiring` supplies operation slots (`zero`, `one`, `add`, `mul`) and
resultless law slots. One closed conformance block binds every inherited slot
to a checked member, an explicit existing-machine reference, or that
conformance's default instantiation. Law members must have checked `ensures`
strong enough to establish the required law. Named conformances disambiguate
multiple algebras
over one carrier. A bare exact-requirement satisfier may serve as an ordinary
lemma or provider realization, but does not assemble a selectable algebra.

The judge may normalize only operations licensed by that conformance. It never
enables algebra by noticing similarly named lemmas in scope. The current
implementation includes licensed associative/commutative rearrangement and a
natural-coefficient polynomial form when the carrier conforms all five
commutative-semiring laws (add/mul commutativity and associativity plus
distributivity). Expansion is capped and unequal normal forms never refute.
Zero/one identities are separate conformed law slots: carrier proofs bridge
their nullary slot applications to constructor constants through ordinary
unfolding and citation. The polynomial normalizer itself does not silently
erase identity terms. Canaries prove that missing conformance disables the
corresponding proof.

Remaining engineering extends the normalized form to full distributive
polynomials, identity bridging, and additional carriers such as `Int`/`Rat`.

Proof-static indexed domains reuse this licensing discipline. A closed index is
just a canonical value. An open symbolic index may normalize only under the
exact selected algebraic conformance and its checked operation contract; a
look-alike operation or unrelated conformance licenses nothing. The first unit
customer needs additive commutative-group normalization for dimension vectors
and multiplicative commutative-group normalization for positive rational
scales. Linear integer arithmetic alone does not normalize symbolic scale
products.

Normalization determines index identity. Established local facts—including a
proof-machine call's checked `ensures`—establish any remaining compatibility
without changing that identity. Indexed domains add no separate proof-citation
surface. Initially, admitted algebraic laws may not license index
normalization: all identity-bearing algebra evidence must be derived. Artifacts
retain the selected algebra-instance and normalized public operation-contract
identity, canonical expression, compatibility evidence, and normalizer
implementation version. The implementation version is provenance metadata, not
part of semantic identity; a canonical-form change is an explicit language
compatibility event.

## Trust and accepted facts

Omega has no `assume` or scattered `unsafe` block. Unproved claims enter through
admission-bearing boundary contracts and root grants, producing explicit trust
receipts only after owner policy accepts them. A bodyless `boundary machine`
carrying `ensures` is an axiom claim, not a proved theorem, as specified by
chapter 10. There is no parallel `boundary fact` spelling. Decision 20's
admission-bearing supply mode remains explicit in the semantic artifact.

One persisted receipt binds the human policy commitment and a domain-separated
digest of its exact subject: selected provider plan, canonical generic machine
template, or checked nongeneric machine contract. Compact report fingerprints
remain visible diagnostics but cannot settle owner admission.

A deferral is different from accepted truth:

- it waives one compiler-generated obligation;
- it creates no reusable fact;
- it is hash-pinned to the obligation site;
- it warns on every build; and
- it is forbidden from crossing a package-release boundary.

Tooling may promote a genuinely permanent deferral into a reviewed boundary
contract. Packages cannot self-grant their own accepted facts.

## Proof kernel and artifacts

The near-term checker may continue to validate proofs directly. The long-term
trust-minimizing path emits derivation records checked by a smaller kernel.
Published proof identity follows the architecture law: deterministic
normalizers own declarations and terms; prover strength gates acceptance but
does not redefine identities.

Artifacts should expose:

- theorem contract identity;
- cited lemmas and accepted premises;
- normalization licenses used;
- derivation/checker version; and
- trust receipts or deferrals in the dependency closure.

## Real-number direction

Runtime floats are fixed-format approximation carriers. Exact proof reasoning
maps them into `FloatMeaning`: finite nonzero values embed in signed rational
arithmetic, while signed zero, infinity, and NaN remain explicit sum cases.
Executable per-operation semantics perform exact rational work plus one format
rounding step on the finite branch. `Real` is a proof-side abstraction built
from ordinary core mathematical data and quotient/equivalence machinery, not a
runtime primitive or compiler float mode.

The useful staging is:

1. exact `Nat`, `Int`, and `Rat` libraries;
2. order and algebraic laws through explicit conformances;
3. signed rational support, `FloatMeaning`, executable operation semantics,
   finite-float embeddings, and error bounds;
4. proof-side `proposition` families, typed index telescopes, and carrierless
   evidence;
5. sequence/Cauchy relation evidence, explicit `Equivalence`, quotient
   formation, and explicitly selected ordinary lifting theorems for `Real`; and
6. approximation theorems connecting `Real` specifications to `f32`/`f64`
   implementations.

Items 4 and 5 are ordered: evidence-bearing quotients cannot land before the
proposition-family/index-telescope fragment. Relation properties are general
mathematical conformances rather than quotient-private proof slots. See
[Law-Bearing Relations, Evidence, And Quotients](law_bearing_relations_and_quotients.md).

## Still open

- derivation-record and small-kernel formats;
- whether reified goal values ever earn a tactic-machine API;
- binder sugar for one-off relational predicates;
- full polynomial normalization and additional algebraic structures; and
- the `Real` library corpus and approximation-policy surface.
