# Design Brief: Mathematical Proofs

Settled direction: 2026-09-07; implementation migration remains open. This is
one proof system expressed through ordinary machines, contracts, data, traits,
and named conformances. Proof-only uses erase after checking.

## Contract-first mathematical proofs

Machines establish facts: their parameters bind subjects, their `requires`
state assumptions, and their `ensures` state conclusions that the body must
justify. A caller must establish the instantiated assumptions before relying
on the conclusions. Naming a condition never proves it, and a failed proof
search is not evidence that the condition is false.

Traits bundle mathematical operations, witnesses, and checked laws. One named
conformance supplies the bundle instead of threading every witness and law
separately. Bundling is the chosen evidence-organization approach, not a proof
that today's trait implementation expresses all mathematics. A bundle's
available laws follow from its checked contract, not from its name.

No dedicated formula declaration is required by the baseline. Optional naming
syntax is an [unproven ergonomic candidate](proof_formula_syntax_candidates.md),
not a prerequisite and not a substitute for general logic. This ruling replaces
the previous declaration-centered design; it does not remove formulas or proofs
from the checker. `Prop` may still name an internal logical sort without becoming
an authored machine result or runtime carrier.

The required mathematical capabilities are independent of that spelling:

- Contracts express universal and existential claims, including nested claims
  and quantification over arbitrary mathematical functions and predicates.
  Quantifying only over executable machine declaration symbols is insufficient.
- Proof-only mathematical values need not be executable. An existence proof
  need not construct a runtime witness. Constructive witnesses can be bundled;
  nonconstructive reasoning and any use of choice retain their assumptions.
- Axioms are explicitly selectable and their transitive dependencies are
  retained. Proof checking establishes a conclusion under those assumptions;
  it does not certify their consistency.
- General mathematics needs a specified account of dependent functions,
  universes, equality, induction, and quotients. Existing specialized contract
  automation is not evidence that this foundation is already implemented.

The source spelling for general logical binders, predicate abstraction and
passing, and noncomputable mathematical values remains to be specified. Support
for different foundations, rather than different axioms within one foundation,
requires a separate explicit treatment of universe, equality, computation, and
proof-irrelevance rules. This ruling does not silently select those rules.

Erased logical facts do not carry consumable authority. A one-shot permission
is an affine or linear Type carrier, possibly with no runtime layout. Witness
values retain their own identity, multiplicity, validity scope, and provenance;
proving equal statements does not identify their chosen witnesses.

## Migration acceptance

Before retiring the implementation's old surface, exercise the replacement on:

1. composition of two witness-and-law bundles, including exact substitution and
   preservation of distinct witnesses;
2. a genuinely higher-order theorem over arbitrary mathematical predicates or
   functions, not an enumeration of executable declarations;
3. nonconstructive existence under an explicit axiom choice, usable in proofs
   but rejected as an executable witness without a constructive implementation;
4. the same theorem checked under accepting and denying axiom policies, with
   transitive assumptions surviving import, erasure, serialization, and replay;
5. a Cauchy/quotient proof and its false twins, preserving representative
   independence and explicit law selection.

Show the actual proof scripts and the checking rules they need. Do not claim
full mathematical expressivity from a mechanical rewrite or a passing simple
example. Migration work and its remaining design dependencies live in
`PROOF-CONTRACT-MIGRATION` in [TASKS.md](../../TASKS.md).

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

- A theorem's machine parameters are checked symbolically. General nested
  quantification and arbitrary mathematical-function binders remain required
  beyond this existing mechanism.
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
General logical binders are a semantic requirement; their exact spelling and
optional shorthand remain design work, not an executable-predicate restriction.

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

Selectable axioms are required, not fixed library truths that every project must
accept. A theorem's published meaning includes its underlying calculus and exact
transitive assumptions. A dependency cannot grant its own assumptions on behalf
of its consumer; replay and reuse must respect the consumer's accepted policy.
An inconsistent axiom set can establish false statements. Checking derivations
neither detects all such inconsistency nor turns an admitted statement into an
assumption-free guarantee. Mathematical admissions must not silently become
runtime-safety or artifact-acceptance authority.

Changing axioms within one calculus is distinct from changing its equality,
universe, or computation rules. Cross-foundation reuse cannot be inferred from
matching printed statements. Foundation identity and compatibility need explicit
design; this is not authorization for an arbitrary checker-plugin mechanism.

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

D40 gives this carrier only structural `FloatMeaningEqual`. Projection already
erases NaN payloads, so NaN is reflexive and signed zeros remain distinct; IEEE
comparison retains its separate opposite laws. A canonical PCC term binds the
verifier-reconstructed float source, format, projection operation, and exact
recognized declaration/catalog contract. Equal keys share one proof identity;
distinct keys need a named theorem. Source spans are provenance, not semantics,
and the complete term erases before runtime.

The useful staging is:

1. exact `Nat`, `Int`, and `Rat` libraries;
2. order and algebraic laws through explicit conformances;
3. signed rational support, `FloatMeaning`, executable operation semantics,
   finite-float embeddings, and error bounds;
4. general proof-side relation expressions, typed index telescopes, and ordinary
   witness-and-law bundles;
5. sequence/Cauchy relation evidence, explicit `Equivalence`, quotient
   formation, and explicitly selected ordinary lifting theorems for `Real`; and
6. approximation theorems connecting `Real` specifications to `f32`/`f64`
   implementations.

Items 4 and 5 are ordered: evidence-bearing quotients cannot land before the
relation-expression/index-telescope fragment. Relation properties are general
mathematical conformances rather than quotient-private proof slots. See
[Law-Bearing Relations, Evidence, And Quotients](law_bearing_relations_and_quotients.md).

## Still open

- derivation-record and small-kernel formats;
- whether reified goal values ever earn a tactic-machine API;
- general logical binders, predicate naming/passing, and noncomputable values;
- universe/equality rules and the scope of multiple-foundation support;
- full polynomial normalization and additional algebraic structures; and
- the `Real` library corpus and approximation-policy surface.
