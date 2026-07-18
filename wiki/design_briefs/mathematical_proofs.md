# Design Brief: Mathematical Proofs

Current as of 2026-07-18. Omega does not introduce a second proof language.
Proofs use ordinary machines, data, contracts, domains, and ranked recursion;
proof-only uses erase after checking.

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
or proof citation when its types and effects permit it. A fact-only invocation
emits no runtime work.

## Quantification and proof data

- Universal claims use machine parameters checked symbolically.
- Element-wise claims use element domains/types.
- Prefix/window claims use bounded views such as `items[0..loaded]`.
- Relational sequence claims use predicate machines plus extraction lemmas.
- Recursive mathematical structures such as `Nat`, `Seq<T>`, `Bag<T>`, and
  exact rational/integer forms are proof-only when they have no finite runtime
  representation.

Proof-only status is structural, not a `[proof]` or `[unbounded]` property.
Anonymous binder syntax remains optional sugar to consider only if named
predicates prove too verbose.

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
therefore obey ranked termination.

## Algebraic canonicalization

Engine-internal normalization is distinct from lemma rewriting. A carrier earns
normalization through explicit conformance to an algebraic trait whose operation
and law requirements are proved.

`CommutativeSemiring` supplies operation slots (`zero`, `one`, `add`, `mul`) and
law slots. Satisfiers bind machines to those slots with `satisfies`; law
satisfiers must have checked `ensures` strong enough to establish the required
law. Named satisfiers disambiguate multiple algebras over one carrier.

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

## Trust and accepted facts

Omega has no `assume` or scattered `unsafe` block. Unproved claims enter through
accepted boundary contracts and root grants, producing explicit trust receipts.
The exact source spelling for an accepted theorem is still open, but the
semantic supply mode is fixed by decision 20.

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

Runtime floats are finite rounded carriers; exact proof reasoning embeds them
in rational arithmetic with explicit rounding/error contracts. `Real` is a
proof-side abstraction built from ordinary core mathematical data and quotient/
equivalence machinery, not a runtime primitive or compiler float mode.

The useful staging is:

1. exact `Nat`, `Int`, and `Rat` libraries;
2. order and algebraic laws through explicit conformances;
3. finite-float-to-rational embeddings and error bounds;
4. sequence/Cauchy/equivalence machinery for `Real`; and
5. approximation theorems connecting `Real` specifications to `f32`/`f64`
   implementations.

## Still open

- accepted theorem and accepted proof-data spelling;
- derivation-record and small-kernel formats;
- whether reified goal values ever earn a tactic-machine API;
- binder sugar for one-off relational predicates;
- full polynomial normalization and additional algebraic structures; and
- the `Real` library corpus and approximation-policy surface.
