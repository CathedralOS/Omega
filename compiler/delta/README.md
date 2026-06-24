# `compiler/delta/` — the certificate checker (the trust anchor)

This is the rung where the lattice's whole thesis — **trust by checking, not by
pedigree** — first becomes concrete. Everything below here (seed, assembler, Beta
compiler) is *plumbing* that gives us a Rust-free language to write this in; *this*
is the artifact that actually decides what is true.

```
check.beta    a minimal natural-deduction proof checker, written in Beta
test.sh       the gate: bc compiles check.beta, then it accepts/rejects certificates
```

## What it checks

Propositional natural deduction over implication and conjunction. By Curry-Howard
the checker *is* a simply-typed lambda-calculus type checker:

| logic | type theory | rule |
| --- | --- | --- |
| proposition `A -> B` | function type | `->`-intro = `lam`, `->`-elim (modus ponens) = `app` |
| proposition `A & B` | product type | `&`-intro = `pair`, `&`-elim = `fst` / `snd` |
| a proof of `A` | a term of type `A` | hypothesis = variable (`hyp`, de Bruijn) |

So "does this certificate prove this proposition?" = "does this term have this
type?", decided by structural type inference (`infer`) + structural equality.

Input (stdin): a goal proposition, then a certificate term, prefix syntax.

```
proposition := UPPERCASE | ( -> prop prop ) | ( & prop prop )
term        := ( hyp N ) | ( lam prop term ) | ( app term term )
             | ( pair term term ) | ( fst term ) | ( snd term )
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

- The logic is propositional (no quantifiers, no `⊥`/negation, no induction). It
  demonstrates the *checker architecture*, not a foundation for real math.
- No **soundness bridge** to program execution — the deep open problem
  (`provable ⟹ true-about-the-Gamma-reference-interpreter`) is untouched. This
  checks proofs *in the calculus*; connecting the calculus to "what a program
  does" is the gamma/delta seam and the core of the proof ambition.
- It is the *reference* checker (small + audited), not a fast one.

Even so: the lattice now has a working checker. The thing the whole architecture
exists to produce has its first, end-to-end-runnable instance.
