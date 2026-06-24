# `compiler/delta/` — the certificate checker (the trust anchor)

This is the rung where the lattice's whole thesis — **trust by checking, not by
pedigree** — first becomes concrete. Everything below here (seed, assembler, Beta
compiler) is *plumbing* that gives us a Rust-free language to write this in; *this*
is the artifact that actually decides what is true.

```
check.beta    a natural-deduction proof checker (validates LOGICAL proofs)
eq.beta       a definitional-equality checker (validates COMPUTATIONAL claims)
test.sh       the gate: bc compiles both, then they accept/reject certificates
```

Two complementary checkers — the two faces a real Delta needs. `check.beta` decides
"does this certificate *prove* this proposition?"; `eq.beta` decides "do these two
terms *compute* to the same value?" (`(p (s (s z)) (s (s z)))` = `(s (s (s (s z))))`,
i.e. 2+2 = 4), by reducing both to normal form with the definitional rules. Its
reducer is **fuel-bounded** (`normalize(t, fuel) -> normal | OutOfFuel`) — the
totality discipline gamma.md/delta.md require, and the bridge from a proposition to
*what a program actually computes* runs through exactly this definitional equality.

## What it checks

Intuitionistic propositional natural deduction over implication, conjunction, and
falsity (so negation is `¬A = A -> ⊥`). By Curry-Howard the checker *is* a
simply-typed lambda-calculus type checker (with a void type):

| logic | type theory | rule |
| --- | --- | --- |
| proposition `A -> B` | function type | `->`-intro = `lam`, `->`-elim (modus ponens) = `app` |
| proposition `A & B` | product type | `&`-intro = `pair`, `&`-elim = `fst` / `snd` |
| proposition `A + B` | sum type | `+`-intro = `inl` / `inr`, `+`-elim = `case` |
| proposition `⊥` | the empty type `Void` | ex falso = `absurd` (a `⊥`-proof yields anything) |
| a proof of `A` | a term of type `A` | hypothesis = variable (`hyp`, de Bruijn) |

So "does this certificate prove this proposition?" = "does this term have this
type?", decided by structural type inference (`infer`) + structural equality.

Input (stdin): a goal proposition, then a certificate term, prefix syntax.

```
proposition := UPPERCASE | ( -> prop prop ) | ( & prop prop ) | ( + prop prop ) | ( bot )
term        := ( hyp N ) | ( lam prop term ) | ( app term term )
             | ( pair term term ) | ( fst term ) | ( snd term )
             | ( inl prop term ) | ( inr prop term ) | ( case term term term )
             | ( absurd prop term )
```

Output: `accept` (exit 1) iff the term proves the goal, else `reject` (exit 0).

```sh
echo '(-> P P) (lam P (hyp 0))' | check     # identity proof of P->P  -> accept
echo '(-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))' | check   # and-elim -> accept
```

`sh test.sh` runs the battery (identity, modus ponens, currying, &-intro/elim,
and-commutativity, function composition all accept; wrong goal, type mismatch,
unbound hypothesis, ill-typed application all reject).

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

## Honest status — this is a Beta *prototype* of Delta

[`rungs/delta.md`](../../wiki/architecture/bootstrap_lattice/rungs/delta.md) says
the checker should be a **Gamma** program — Gamma's algebraic data types + pattern
matching are what keep such a checker small and auditable. Gamma doesn't exist
yet, so this is written in Beta, and it *shows exactly why Gamma is wanted*: the
term/type trees are hand-encoded as tagged 3-word nodes in raw memory, and `infer`
is an if-cascade on integer tags — precisely the boilerplate sum types + pattern
matching would erase. So this prototype is also the design pull for the Gamma rung.

What it is **not** (yet), all tracked in `rungs/delta.md`:

- The logic is *full intuitionistic propositional* (`->`, `&`, `+`, `⊥`/negation)
  — but no quantifiers and no induction, so it demonstrates the *checker
  architecture*, not yet a foundation for real math. First-order quantifiers and
  an inductive `Nat` are the natural next additions.
- No **soundness bridge** to program execution — the deep open problem
  (`provable ⟹ true-about-the-Gamma-reference-interpreter`) is untouched. This
  checks proofs *in the calculus*; connecting the calculus to "what a program
  does" is the gamma/delta seam and the core of the proof ambition.
- It is the *reference* checker (small + audited), not a fast one.

Even so: the lattice now has a working checker. The thing the whole architecture
exists to produce has its first, end-to-end-runnable instance.
