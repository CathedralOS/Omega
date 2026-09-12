# W-based inductive profile

## Scope and checking authority

This is the selected extension of the [predicative reference core](foundation.md).
It adds typed function eta, relevant identity, a two-element type and W-types.
Indexed families are checked derived constructions, not a second primitive
inductive-declaration mechanism. It adds no source keyword, anonymous machine
or authored `-> Prop`.
Source-level mathematical binders remain [separately open](../../../OWNER_QUESTIONS.md#mathematical-binders).

The rules below are requirements, not evidence of completed implementation or
metatheory. `PROOF-KERNEL-CORE` owns their checking and justification;
`PROOF-CONTRACT-MIGRATION` owns source integration. A complete verified-profile
claim requires those obligations, not merely a profile name or successful examples.

All notation here is kernel mathematics. `=` below is the relevant identity
type; `≡` is definitional conversion. Level variables are explicit and checked.
The reference core's formation, substitution, conversion and strict-layer rules
remain in force. These additions grant no universe resizing or equality reflection.

## Typed function eta

Extend conversion with dependent-function eta at one checked function type:

```text
Γ ⊢ f : Π(x : A). B(x)             x is fresh for Γ and f
────────────────────────────────────────────────────────
Γ ⊢ (x ↦ f(x)) ≡ f : Π(x : A). B(x)
```

The premise includes well-formedness of the dependent function type. Both
terms must check at that same type, with the same domain, dependent codomain and
level application. The rule applies to every Π sort combination admitted by
the reference core; it creates no additional function types or elimination
permissions. When this function type is strict, equality of its inhabitants is
already covered by irrelevance.

Typedness is load-bearing. This is a conversion judgment, not an unrestricted
syntactic rewrite deleting any wrapper shaped like `x ↦ f(x)`. It grants neither
function extensionality (equality from pointwise proofs), equality reflection,
K/UIP nor new mathematical assumptions. Assumption closure remains independent
of conversion. No source lambda, anonymous machine, function-pointer identity
rule or runtime optimization is introduced.

This is an explicit extension to the reference core's function beta and pair eta.
Its combined justification must cover substitution, preservation, normalization
and decidable conversion, including relevant/strict contexts and checked levels.
The conversion algorithm must be justified against the typed judgment; success
in another proof assistant is not that justification.

## Primitive rules

| Former | Formation and introduction | Elimination and computation |
| --- | --- | --- |
| `Id` | For `A : Type u` and `x,y : A`, `Id A x y : Type u`; `refl x : Id A x x`. | Dependent identity elimination `J`, computing only on reflexivity. No K/UIP or identity-proof irrelevance. |
| `Two` | `Two : Type 0`, with `zero, one : Two`. | Dependent case analysis, computing to the corresponding branch on each constructor. |
| `W` | For `A : Type u` and `B : A → Type v`, `W(A,B) : Type max(u,v)`; `sup(a,k)` for `a : A`, `k : B(a) → W(A,B)`. | Dependent induction with an induction hypothesis for every child, computing on `sup`. |

For fixed `A` and `x : A`, identity elimination takes
`C : (y : A) → Id A x y → Type w` and `d : C(x,refl x)`:

```text
J(C,d,y,p) : C(y,p)
J(C,d,x,refl x) ≡ d
```

For `C : Two → Type w`, `d0 : C(zero)` and `d1 : C(one)`:

```text
caseTwo(C,d0,d1,t) : C(t)
caseTwo(C,d0,d1,zero) ≡ d0
caseTwo(C,d0,d1,one)  ≡ d1
```

For `P : W(A,B) → Type w`, the induction step has type:

```text
step : (a : A) → (k : B(a) → W(A,B))
       → ((b : B(a)) → P(k(b))) → P(sup(a,k))

indW(P,step,t) : P(t)
indW(P,step,sup(a,k)) ≡ step(a,k,b ↦ indW(P,step,k(b)))
```

The motive level `w` is independently checked; elimination is not confined to
the input's level. The new primitive eliminators above target relevant `Type`.
Strict targets are handled through the existing `Box`/unboxing rules. Add no
new definitional eta rule for `Two`, `Id` or `W`. There is no recursion equation
for arbitrary axiomatic terms and no general user-authored rewrite rule.

Relevant empty and unit types are derived from the reference core's strict
empty/unit and boxing. Constructor alternatives use `Two` and dependent sums;
records use dependent sums. These mathematical encodings do not select native
tags, memory layout or nominal Omega type identity.

## Derived indexed families

The selected construction uses unindexed trees with a recursively defined
indexing condition. Given:

```text
I : Type l                    indices
A : Type u                    constructor labels and payloads
B : A → Type v                child positions
out : A → I                   index at a node
next : (a : A) → B(a) → I      required index of each child
```

define by W-induction, then dependent pairing:

```text
IndexedAt(i, sup(a,k)) =
    Id I (out(a)) i × ((b : B(a)) → IndexedAt(next(a,b), k(b)))

IW(i) = Σ (t : W(A,B)). IndexedAt(i,t)
```

The displayed `=` introduces definitions, not equality-reflection rules.
`IW(i)` fits `Type max(l,u,v)`; any lift needed by the core's universe rules is
explicit. Constructor and dependent induction terms must be derived with their
constructor computation judgment checked definitionally, not postulated as a law.
The indexing evidence is relevant identity, not a squashed substitute.

The [checked HoTT construction](https://hott.github.io/Coq-HoTT/coqdoc-html/HoTT.Types.IWType.html)
is a construction reference, not an imported acceptance verdict. Its basic
encoding/induction precedes the function-extensionality-dependent equivalence
theorem. Do not import that extra assumption into the basic construction.
Transplantation must justify the rules in this exact theory, including strict
contexts, universes, substitution and conversion.

The construction rebuilds a child function as `b ↦ (fst(g(b)),snd(g(b)))`.
Pair eta identifies this with `b ↦ g(b)`; the selected typed function eta closes
it to `g`, even when `g` is an arbitrary variable. This conversion dependency
and its justification cost are part of the profile, not an imported Coq default.
Concrete instances may already compute without this step; the requirement is
the general dependent computation judgment. Merely replacing that judgment with
a propositional law would change the contract, and such a law itself needs proof.

## Declaration correspondence and strict logic

Source declarations elaborate to checked terms and constructor/eliminator
applications. A source positivity check may diagnose an invalid declaration;
its success does not authorize a new kernel rule. A well-typed W term may encode
the wrong source declaration. Therefore both are required:

- A justification of the encoding scheme preserving formation, constructors,
  dependent induction and computation.
- Independently checked application of that scheme to the exact declaration,
  including parameters, indices, payloads, case constraints and recursive uses.

Round-trip decoding or structural comparison may check retained structure; it
does not prove semantic preservation. Primitive inductives would also require
source elaboration correctness. No trust-class improvement follows merely from
moving a check out of the kernel. Statement fidelity, nominal identity, ownership
and lowering correspondence remain their existing owners' obligations.

There is no second general strict-inductive primitive. Logical inductive
relations may use a relevant derivation family and `Squash` when their intended
meaning is existence without witness access. Stronger eliminators require checked
constructions from the selected rules, not a constructor-count exception. The
reference paper's section 5 is not an automatic elimination permission.

Diagnostics distinguish an invalid declaration, an unsupported valid encoding
and malformed compiler-generated evidence. Report source-level failures at their
declaration/field/index occurrence; do not turn encoding failure into a source
positivity verdict or expose only an internal W-unification error.

## Required evidence and measurements

Before claiming coverage, demonstrate vectors indexed by length, derivations
indexed by context and conclusion, mutual families and nested strictly-positive
data. Include case type equalities and payload couplings. Check dependent
induction and constructor computation, plus negative recursion, invalid universe
constraints and unauthorized strict elimination. The current proof-rule count
is not the definition of the derivation-family customer.

For constructor computation, expose the constructor but keep its supplied child
function arbitrary (neutral), not only an already-expanded lambda. Include
dependent motives, fresh-binder renaming and all admitted Π sort combinations.
Reject mismatched function types and variable capture; eta alone must not turn
pointwise equality evidence into function equality. The eliminator need not
reduce on an entirely neutral tree. Successful concrete evaluation is not a
substitute for the general constructor-computation check.

Justification includes the added kernel rules, the indexed scheme and its
applications; no single example discharges preservation, conversion decidability
or implementation correctness. Record term size, retained storage and checking
cost in the environment actually checking application proofs. Bootstrap costs
need a separately named bootstrap customer; nested interpretation is not the
default application cost model.

W is selected now. Reopen only on demonstrated coverage/computation failure,
unacceptable measured cost or established audit burden, not speculative primitive
counts, a difficult proof attempt or first-implementation slowness. Such a change
belongs in `OWNER_QUESTIONS.md`; it is not an automatic fallback.
