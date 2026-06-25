# `compiler/delta/` — the certificate checker (the trust anchor)

This is the rung where the lattice's whole thesis — **trust by checking, not by
pedigree** — first becomes concrete. Everything below here (seed, assembler, Beta
compiler) is *plumbing* that gives us a Rust-free language to write this in; *this*
is the artifact that actually decides what is true.

```
check.beta           a natural-deduction proof checker (validates LOGICAL proofs)
eq.beta              a definitional-equality checker (validates COMPUTATIONAL claims)
test.sh              the gate: bc compiles both, then they accept/reject certificates
soundness.sh         adversarial battery — invalid certificates must ALL be rejected
checker-diamond.sh   diverse double-check: check.beta vs checker.gamma must agree
semantics-diamond.sh definitional equality vs the interpreter's operational eval
```

Two complementary checkers — the two faces a real Delta needs. `check.beta` decides
"does this certificate *prove* this proposition?"; `eq.beta` decides "do these two
terms *compute* to the same value?" (`(p (s (s z)) (s (s z)))` = `(s (s (s (s z))))`,
i.e. 2+2 = 4), by reducing both to normal form with the definitional rules. Its
reducer is **fuel-bounded** (`normalize(t, fuel) -> normal | OutOfFuel`) — the
totality discipline gamma.md/delta.md require, and the bridge from a proposition to
*what a program actually computes* runs through exactly this definitional equality.

## What it checks

Full intuitionistic **first-order** predicate logic: the propositional connectives
(implication, conjunction, disjunction, falsity — so negation is `¬A = A -> ⊥`),
equality of Peano terms with a computation-aware `refl`, and the quantifiers `∀`/`∃`
over individuals with unary and binary atomic predicates. By Curry-Howard the
checker *is* a dependently-flavoured lambda-calculus type checker:

| logic | type theory | rule |
| --- | --- | --- |
| proposition `A -> B` | function type | `->`-intro = `lam`, `->`-elim (modus ponens) = `app` |
| proposition `A & B` | product type | `&`-intro = `pair`, `&`-elim = `fst` / `snd` |
| proposition `A + B` | sum type | `+`-intro = `inl` / `inr`, `+`-elim = `case` |
| proposition `⊥` | the empty type `Void` | ex falso = `absurd` (a `⊥`-proof yields anything) |
| proposition `t1 = t2` | identity type (over Peano) | `refl` — valid iff `t1 ≡ t2` *by reduction* (the conversion rule) |
| proposition `∀x. P` | dependent function | `∀`-intro = `gen` (freshness-guarded), `∀`-elim = `inst` (capture-avoiding) |
| proposition `∃x. P` | dependent pair | `∃`-intro = `wit` (witness + proof), `∃`-elim = `unpack` |
| a proof of `A` | a term of type `A` | hypothesis = variable (`hyp`, de Bruijn) |

Individuals range over the two built-in inductive types — Peano naturals (`z`, `s`,
`+`, `*`) and Lists (`nil`, `cons`, `++`, with `append`/`len` computing under the
conversion rule) — **and over user-declared inductive types**: `(data cid arity r0 r1)`
declares a constructor's shape, `(k cid args…)` builds inert structural-equality data,
and `(rec ca cb motive caseA caseB)` is general structural induction over that type,
subsuming the `natind`/`listind` shapes (e.g. a binary `Tree` of `Leaf`/`Node`).
There are also de Bruijn individual variables `(v k)`. `gen` introduces a fresh variable and is guarded so a variable free
in an open hypothesis cannot be captured, while `inst` substitutes capture-avoidingly
(de Bruijn shifting), so the full instantiation laws — e.g. `∀x.∀y.R(x,y) -> ∀z.R(z,z)`
— hold. Predicate arguments are compared up to the conversion rule, so `P(1+1)` and
`P(2)` are the same proposition.

`check.beta` now spans both faces: it proves logical propositions **and**, via
`refl` + the conversion rule, computational equations like `2+2 = 4` — which are
themselves first-class propositions that combine with `-> & + ⊥`. `refl` is
discharged by normalizing both sides (the same fuel-bounded reducer as `eq.beta`),
so "logic meets computation" — the gamma/delta soundness seam — is concrete here.

So "does this certificate prove this proposition?" = "does this term have this
type?", decided by structural type inference (`infer`) + structural equality.

Input (stdin): zero or more declarations — `( def N prop term )` lemmas and
`( data cid arity r0 r1 )` constructors — then a goal proposition and a certificate.

```
proposition := UPPERCASE | ( -> prop prop ) | ( & prop prop ) | ( + prop prop ) | ( bot )
             | ( = nat nat ) | ( All prop ) | ( Exists prop )
             | ( Pred id nat ) | ( Rel id nat nat )
term        := z | ( s term ) | ( p term term ) | ( m term term ) | ( v k )  ; Peano + indiv var
             | nil | ( cons term term ) | ( app term term ) | ( len term )   ; Lists (compute)
             | ( k cid term… )                                               ; user constructor
term        := ( hyp N ) | ( lam prop term ) | ( app term term )
             | ( pair term term ) | ( fst term ) | ( snd term )
             | ( inl prop term ) | ( inr prop term ) | ( case term term term )
             | ( absurd prop term ) | ( refl nat )
             | ( gen term ) | ( inst term nat ) | ( wit prop nat term ) | ( unpack term term )
             | ( natind prop term term )    ; Peano induction: base P(0), step ∀n.P(n)->P(s n)
             | ( listind prop term term )   ; list induction: base P(nil), step ∀h t.P(t)->P(cons h t)
             | ( rec ca cb prop term term ) ; general structural induction over a declared type
             | ( eqelim prop term term )    ; Leibniz: motive P, pf of a=b, pf of P(a) -> P(b)
             | ( disj term ) | ( sinj term ) | ( use N )  ; no-confusion ; cite lemma N
```

Output: `accept` (exit 1) iff the term proves the goal, else `reject` (exit 0).

```sh
echo '(-> P P) (lam P (hyp 0))' | check     # identity proof of P->P  -> accept
echo '(-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))' | check   # and-elim -> accept
# ∀x.(P x -> Q x) -> (∀x.P x -> ∀x.Q x)   (first-order distribution)
echo '(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 0 (v 0))) (All (Pred 1 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 0 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))' | check   # -> accept
```

`sh test.sh` runs the battery across the whole logic — propositional (modus ponens,
currying, and-commutativity, or-elimination, composition, ex falso), arithmetic
(`2+2=4`, `2*3=6` by computation), and first-order (∀-distribution, ∃-intro/elim,
instantiation under nested quantifiers, binary relations) — with matched accept/reject
pairs. Three further gates harden the trust anchor:

- `sh soundness.sh` — a battery of *invalid* certificates that must all be rejected,
  including classical tautologies (excluded middle, double-negation, Peirce, the
  drinker paradox) that have no constructive proof.
- `sh checker-diamond.sh` — the same proofs through **two independent checkers**
  (`check.beta` and `gamma/checker.gamma`); every verdict must agree.
- `sh semantics-diamond.sh` — the checker's definitional `=` vs the interpreter's
  operational evaluation, agreeing on every equation (the gamma/delta seam).

## The full stack

`check` is compiled by **bc** (the self-hosting Beta compiler — no Rust in its
execution), assembled by the alpha assembler, and run on the hand-audited seed:

```
hand-audited alpha seed
  runs the assembler (written in alpha)
  which lowered bc (the Beta compiler, written in Beta)
  which compiled check.beta (this checker)
  which validates a certificate -> accept / reject
```

A separate, untrusted, arbitrarily-clever proof-**search** engine may produce
certificates; it has no authority — a false proposition cannot get past `check`,
however the certificate was found. That asymmetry (tiny trusted checker, unbounded
untrusted producer) is the entire point.

## Status — diverse, type-checked, adversarially tested

[`rungs/delta.md`](../../wiki/architecture/bootstrap_lattice/rungs/delta.md) says the
checker should be a **Gamma** program — Gamma's algebraic data types + pattern
matching keep such a checker small and auditable. That now exists *both* ways, and
the difference is the point:

- `check.beta` (this file) hand-encodes the term/type trees as tagged 3-word nodes in
  raw memory and decides everything with a CFG guard-state dispatch on integer tags
  (Beta is itself state-graph shaped, no if/while) — exactly the boilerplate that
  motivated the Gamma rung.
- [`gamma/checker.gamma`](../gamma/checker.gamma) is the *same logic* as a dozen tiny
  functions over algebraic data + pattern matching. `checker-diamond.sh` runs proofs
  through **both** and requires identical verdicts — two independent implementations,
  in different languages at different rungs, cross-checking the trust anchor
  (diversity = security, applied to the checker itself).
- [`gamma/checker_typed.gamma`](../gamma/checker_typed.gamma) is that Gamma checker
  fully annotated, and Gamma's own static type checker (`gamma/typeck.beta`) accepts
  it — so the trust anchor's *code* is shown statically type-safe.

The logic is now **first-order intuitionistic predicate logic with induction**: all
propositional connectives, equality with the conversion rule, `∀`/`∃` with
capture-avoiding instantiation over unary and binary predicates, induction over the
two built-in inductive types (`natind`, `listind`) **and over user-declared types**
(`rec`, general structural induction), **Leibniz equality elimination** (`eqelim`,
the identity-type `J` / transport), and **named lemmas** (`def`/`use`) so big proofs
factor instead of forming one monolith. Together those prove genuine theorems — from
`∀n. n+0 = n` and `∀l. l++nil = l` (induction + `eqelim`) up to **addition
commutativity and right distributivity** `(a+b)*c = a*c + b*c` (the Peano naturals
satisfy the core semiring axioms), and the induction principle of an arbitrary
user-declared datatype. Every soundness-critical rule is *adversarially* tested:
`gen`'s freshness guard and `inst`'s de Bruijn shifting against a capture
discriminator, and `natind`/`listind`/`rec` against an identity-shaped step a broken
rule would use to derive `∀x.P(x)` from the base alone. `soundness.sh` confirms no
false certificate — nor any non-constructive classical tautology — gets through.

The Peano axioms are now complete enough to refute as well as prove: `disj`
(no-confusion, `0 ≠ s n`) and `sinj` (injectivity of the successor) are sound
primitive rules, so `¬(0 = 1)` and `s n = s 0 ⊢ n = 0` check.

What it is **not** (yet), tracked in `rungs/delta.md`:

- **No user-defined recursive functions** over user types. A declared type's
  constructors are inert data with structural equality; you can do induction over a
  `Tree`, but there is no `mirror`/`size` with reduction rules feeding the conversion
  rule. That function-definition layer is the next frontier and what would make
  *theorems* (not just induction principles) over user types provable.
- **No soundness bridge** to program execution. `semantics-diamond.sh` *exhibits* the
  gamma/delta seam (the checker's definitional `=` agreeing with the interpreter's
  operational evaluation), but the theorem `provable ⟹ true-about-the-reference-
  interpreter` is the deep open problem, untouched.
- **`unpack` (∃-elim) is conservatively incomplete.** It rejects any conclusion `C`
  that mentions *any* individual variable, when soundness only needs `C` to avoid the
  *witness* variable (the one the handler's `∀` binds). So a `C` that legitimately
  mentions an *outer* quantified variable is refused — which blocks the constructive
  quantifier interchange `∃y.∀x.R(x,y) → ∀x.∃y.R(x,y)`. The sound fix is to replace the
  blanket `ivar_in_prop(C)` check with "the witness var is not free in `C`" and to
  shift `C` down past the removed binder; it is a small but soundness-critical change
  (it must keep the capture discriminator and the whole soundness battery green).
- It is the *reference* checker (small + audited), not a fast one.

The thing the whole architecture exists to produce — a tiny, hand-auditable checker
with sole authority over what is true — is here, end-to-end runnable on the seed,
proving real arithmetic by induction, and double-checked by an independent twin.
