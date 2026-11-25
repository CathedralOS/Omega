# Unified native inductive profile

## Scope and checking authority

Omega selects unified native inductive declarations (Option C) for the
mathematical kernel. Ordinary recursive `data`, indexed families and mutually
recursive declarations use the same checked declaration mechanism. For example,
core declares `Nat` with ordinary `Zero` and `Succ(prev: Nat)` cases. The
kernel obtains induction from those constructors. No boundary declaration,
runtime representation or foreign binding is involved.

The [predicative reference core](foundation.md) is extended with typed function
eta, relevant identity, the two-element type and checked inductive declarations.
Source binders follow the [mathematical source contract](mathematical_bindings.md).

A complete verified-profile claim requires checked rules, their combined
metatheoretic justification and faithful source integration. A profile name or
successful examples do not establish those obligations.

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

The motive level is independently checked. Relevant identity and two-element
elimination target relevant `Type`. Strict targets use the existing
`Box`/unboxing rules. Relevant empty and unit types can still be derived from
strict empty/unit and boxing.

<a id="derived-indexed-families"></a>

## Native inductive declarations

The kernel checks each declaration block before extending its signature with
the families, constructors and their dependent induction principles (recursors).
This applies equally to a plain type, a type with uniform parameters such as
`Seq<T>`, a family indexed by values such as `Vector<Length, Total>`, and a
mutual block. Each constructor must return the declared family at the declared
uniform parameters and well-typed result indices. Check the payload types,
universe bounds and strict positivity of all recursive occurrences across the
whole block. A recursive occurrence as the input to a stored function is a
standard negative case.

The kernel derives each recursor's type and constructor computation rules from
the admitted block. Eliminating a constructor computes its corresponding branch
with the constructor arguments and the induction hypotheses for its recursive
children at their actual indices. For `Nat`, the `Zero` branch receives no
induction hypothesis and the `Succ(prev)` branch receives one for `prev`.
For a mutual family, each hypothesis uses its child's family and motive.
An application to a neutral subject may stay neutral. A producer cannot supply
a new reduction rule by naming a recursor in a proof artifact.

## Declaration correspondence and strict logic

Elaboration must preserve the source declaration's nominal identity, binders,
parameters, indices, payloads, recursive uses and case-conditional default-domain
predicates. Those predicates retain their ordinary construction and elimination
obligations. Independently check the correspondence between the source and
kernel declarations. Kernel admission alone proves only the declaration supplied
to it. Data layout, ownership and source visibility follow their existing rules.

Inductive families inhabit relevant `Type`. Logical derivations can use a
relevant family and `Squash` when only existence is observable. Native induction
adds no route for extracting a relevant witness from strict evidence. Diagnostics
identify the offending declaration, field or index and distinguish invalid
source from an unsupported elaboration or malformed compiler evidence.

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

Justify substitution, preservation, normalization and decidable conversion for
the admitted blocks and generated reductions together with the rest of the
calculus. The implementation's existing [W/IW metatheory](kernel_metatheory.md)
documents its earlier encoding. The [kernel prototypes](../../drafts/designs/recursive_data_encoding_experiments.md)
supply comparative evidence. End-to-end acceptance requires checking authored
declarations through elaboration and Psi publication/consumption under the
existing admission rules. Retire the production W/IW encoding once this path
preserves constructor computation, induction, index checks and refusal of
malformed declarations and forged recursors. Track conversion steps, allocation
and checking time on the same declarations and proofs during migration.
