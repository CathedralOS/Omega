# `source/alpha/checker/` — Alpha certificate checker

This Alpha-owned service is where the lattice's whole thesis — **trust by
checking, not by pedigree** — becomes concrete. The bootstrap spine gives us a
Rust-free route to run its current implementations.

The authoritative Beta checker artifact is compiled by the Alpha-written cold
Beta compiler, below `bc`, and reconstructed byte-for-byte by
`reconstruct-artifact.sh`. A second checker compiled by `bc` remains useful
regression and differential evidence, but cannot admit its own producer.

This tree is checker infrastructure rather than a compiler or language rung.
Canonical callers resolve the `alpha-checker` role through `tools/lattice/paths.sh`.
Internal ownership is split by responsibility; the host language of a file does
not determine its owner.
See the
[repository structure](../../../wiki/architecture/bootstrap_lattice/repository_structure.md).

```
implementations/
  beta/              lattice-built logical and definitional-equality checkers
  reference/         independent executable reference checker
  gamma/             independent Gamma checker and its typed form
artifacts/            persisted below-Beta checker tape and rebuild entry
artifact_env.sh       stamps the accepted tape into the audited host seed
construct-artifact.sh direct Alpha -> cold Beta -> checker construction
tools/                untrusted elaboration, proof search, and certificate conversion
corpus/               proof sources, shared libraries, and deterministic fuzz generators
gates/                executable soundness, cross-check, and operational-seam policy
```

Only checker implementations decide derivation validity. `tools/`, `corpus/`, and
`gates/` may construct, translate, perturb, or replay evidence, but have no authority
to make a certificate valid.

D10 selects `.proof` for proof-source files. The atomic migration updated all
254 sources present at its landing commit, plus their generators, gates, and
references. This is a source-file convention; it does not change the raw
certificate format or make the elaborator trusted.

Two complementary checkers — the two faces a real proof kernel needs. `check.beta` decides
"does this certificate *prove* this proposition?"; `eq.beta` decides "do these two
terms *compute* to the same value?" (`(p (s (s z)) (s (s z)))` = `(s (s (s (s z))))`,
i.e. 2+2 = 4), by reducing both to normal form with the definitional rules. Its
reducer is **fuel-bounded** (`normalize(t, fuel) -> normal | OutOfFuel`) — the
totality discipline the Gamma and proof-kernel architecture require, and the bridge from a proposition to
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
so "logic meets computation" — the proof/meaning soundness seam — is concrete here.

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

`sh gates/test.sh` runs the battery across the whole logic — propositional (modus ponens,
currying, and-commutativity, or-elimination, composition, ex falso), arithmetic
(`2+2=4`, `2*3=6` by computation), and first-order (∀-distribution, ∃-intro/elim,
instantiation under nested quantifiers, binary relations) — with matched accept/reject
pairs. Three further gates harden the trust anchor:

- `sh gates/soundness.sh` — a battery of *invalid* certificates that must all be rejected,
  including classical tautologies (excluded middle, double-negation, Peirce, the
  drinker paradox) that have no constructive proof.
- `sh gates/checker-diamond.sh` — the same proofs through **two independent checkers**
  (`implementations/beta/check.beta` and `implementations/gamma/checker.gamma`);
  every verdict must agree.
- `sh gates/semantics-diamond.sh` — the checker's definitional `=` vs the interpreter's
  operational evaluation, agreeing on every equation (the proof/meaning seam).

## The full stack

The authoritative `check` tape is constructed below **bc**, then stamped into
the hand-audited seed:

```
hand-audited alpha seed
  runs the assembler (written in alpha)
  which lowers the cold Beta compiler (written in alpha)
  which compiles check.beta (this checker) to artifacts/proof_checker_bytecode.tape
  which validates a certificate -> accept / reject
```

`reconstruct-artifact.sh` builds that tape once below Beta and compares it
byte-for-byte with the committed artifact before running accept/reject controls.
A second identical construction is reproducibility evidence, not another
checker-lineage premise. The accepted `beta_compiler_bytecode.tape` may separately compile the
same checker source as differential evidence, but that product cannot admit the
compiler that produced it.

A separate, untrusted, arbitrarily-clever proof-**search** engine may produce
certificates; it has no authority — a false proposition cannot get past `check`,
however the certificate was found. That asymmetry (tiny trusted checker, unbounded
untrusted producer) is the entire point.

## Status — cross-checked, type-checked, adversarially tested

[`proof_kernel.md`](../../../wiki/architecture/bootstrap_lattice/proof_kernel.md)
places the checker outside the language spine and requires independent low-rung
implementations. Gamma's algebraic data types + pattern matching keep one such
implementation small and auditable. The implementations provide useful
bug-finding evidence while the soundness bridge matures:

- [`implementations/beta/check.beta`](implementations/beta/check.beta) hand-encodes the term/type trees as tagged 3-word nodes in
  raw memory and decides everything with a CFG guard-state dispatch on integer tags
  (Beta is itself state-graph shaped, no if/while) — exactly the boilerplate that
  motivated the Gamma rung.
- [`implementations/gamma/checker.gamma`](implementations/gamma/checker.gamma) is the *same logic* as a dozen tiny
  functions over algebraic data + pattern matching. `gates/checker-diamond.sh` runs proofs
  through **both** and requires identical verdicts. This agreement does not
  itself prove either checker sound.
- [`implementations/gamma/checker_typed.gamma`](implementations/gamma/checker_typed.gamma) is that Gamma checker
  fully annotated, and Gamma's own static type checker (`../../gamma/typeck.beta`) accepts
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

What it is **not** (yet), tracked in the architecture's `proof_kernel.md`:

- **User-defined recursive functions** over user types are now **implemented** (arity
  0/1 constructors), per [`FUNCTIONS.md`](FUNCTIONS.md). A `(fun FID CID body)` rule
  per constructor (`(f FID t)` applies, `(rec 0)` recurses) reduces under `normalize`,
  so its equations feed the conversion rule: e.g. a function `g` embedding a user-`Nat`
  into Peano computes `g(S Z) = s z` and proves the universal law `∀n. g(S n) = s(g n)`.
  Totality is free — `normalize` is fuel-bounded, so a divergent definition just stays
  STUCK. Present in all three checkers — check.beta (a `(fid,cid)` table), checker.gamma
  (rules inline on the term, **diamond-cross-checked**), checker_typed.gamma (typed) —
  each storing rules its own way without growing a second responsibility, and the
  soundness battery pins that open/no-rule applications stay stuck. **Still open:**
  arity-2 (e.g. a `Tree` fold) and the eq.beta/interp semantics-diamond mirror.
- **No soundness bridge** to program execution. `semantics-diamond.sh` *exhibits* the
  proof/meaning seam (the checker's definitional `=` agreeing with the interpreter's
  operational evaluation), but the theorem `provable ⟹ true-about-the-reference-
  interpreter` is the deep open problem, untouched.
- It is the *reference* checker (small + audited), not a fast one.

The small generic checker is here, end-to-end runnable on the seed, proving real
arithmetic by induction and double-checked by an independent twin. It has sole
authority over derivation validity, while the separate artifact-aware ledger
reconstruction determines which propositions a deployed program must prove.
