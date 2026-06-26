# `compiler/` — the bootstrap lattice

A tower of small languages rising from a tiny, hand-audited seed. Its thesis is
**trust by checking, not by pedigree**: nothing is trusted because of where it came
from (a vendor, a binary, a previous compiler); each rung's output is *checked* —
re-derived, conformance-tested, or diverse-double-checked — so trust is earned, not
assumed. The whole tower verifies in one command:

```sh
sh verify-lattice.sh        # seed → assembler → bc → checker, every rung's gate in order
```

## The rungs

Each rung is built by the one below it and pinned by its own gate. The lower you go,
the smaller and more auditable; the higher you go, the more expressive.

| rung | what it is | trust mechanism |
| --- | --- | --- |
| **alpha** | a ~300-line seed VM (21 opcodes), the root of trust | hand audit + a 25-case conformance suite (`SEMANTICS.md`) + a **diamond**: two independent seeds (x64, arm64) emit byte-identical bytecode |
| **beta** | the assembler, written in Alpha | **self-hosts** — reproduces its own bytecode byte-for-byte |
| **beta-lang-rs** | a throwaway Rust on-ramp for the Beta language | exists only to bootstrap `bc`; then leaves the lineage |
| **beta-lang** (`bc`) | the Beta compiler **written in Beta** | self-hosts byte-for-byte — Rust is out of the trust path |
| **gamma** | a safe functional language (ADTs + pattern matching): a reference interpreter (`interp.beta`) and a static type checker (`typeck.beta`) | interpreter + 22-case type-checker gate; the type checker is what makes the checker *safe to write* |
| **delta** | the **certificate checker** — the trust anchor | see below |
| epsilon, omega | higher rungs (systems language; full dependent types) | design-stage |

Trust flows bottom-up: the hand-audited seed runs the assembler, which lowers `bc`,
which compiles the checker, which validates a proof. No rung trusts its builder
blindly — every step is re-checkable.

## The trust anchor (`delta/`)

This is the rung the whole architecture exists to produce: a small, hand-auditable
checker with **sole authority over what is true**. An untrusted, arbitrarily clever
proof-*search* engine may produce certificates; this checker decides — a false
proposition cannot get past it, however the certificate was found.

`check.beta` now decides **first-order intuitionistic predicate logic with
induction**:

- propositional logic (`→ & + ⊥`), so `¬A = A → ⊥`, by Curry-Howard a simply-typed
  λ-calculus type checker;
- **equality** with the conversion rule (`refl` discharged by computation) — an
  equivalence relation (symmetry/transitivity via Leibniz `eqelim`);
- `∀`/`∃` with **capture-avoiding** instantiation (de Bruijn shifting), unary and
  binary predicates; `∀`-introduction lifts the hypothesis context one binder (the
  eigenvariable condition holds structurally), so `∃`-**elimination works under open
  hypotheses** — enough to define `a ≤ b := ∃c. a+c=b` and prove it a **total (linear) order**
  (reflexive, transitive, antisymmetric, total) inside the checker, with additive cancellation
  (`a+x=a+y → x=y`) along the way;
- induction over the two built-in inductive types (Peano naturals, Lists) **and over
  user-declared types** (`data` + `rec` — general structural induction, e.g. a binary
  `Tree`), plus Peano no-confusion (`disj`, `sinj`); and, derived from these, **strong
  (course-of-values) induction** `(∀n. (∀m<n. P m) → P n) → ∀n. P n` for an *abstract*
  predicate — the well-founded-induction principle for `<`;
- **named lemmas** (`def`/`use`) so proofs factor instead of forming one monolith;
- **user-defined recursive functions** over declared types (`fun` rules; `(f …)` applies,
  `(rec i)` recurses; up to 2 arguments, functions may call functions) whose equations
  reduce under the conversion rule — so theorems *about user functions* prove by
  induction: a user-defined binary `add` is a **commutative monoid** (`∀x. add(x,Z)=x`,
  associativity, `∀x∀y. add(x,y)=add(y,x)`) and a `mult` *defined via* `add` both
  **distributes** over it (`mult(a+b,c)=mult(a,c)+mult(b,c)`) and **commutes**
  (`mult(a,b)=mult(b,a)`) — so a number type with USER-DEFINED arithmetic is a full
  **commutative semiring** inside the checker, the same axioms built-in `+`/`*` meet;
- real theorems, all pinned in the gate: `n+0=n`, `n≠s n`, every nat is `0` or a
  successor, `l++nil=l`, append associativity, `len(a++b)=len(a)+len(b)`, and — via
  lemmas — **addition *and* multiplication commutativity**, **both distributive laws**
  (`(a+b)*c` and `a*(b+c)`), **both associativities** (`(a+b)+c` *and* `(a*b)*c`), and the
  `0`/`1` identities, so the built-in naturals are a full **commutative semiring** inside the
  checker (the same status shown for a user-*defined* number type); plus pure-logic
  theorems — non-contradiction `¬(A&¬A)`, the contrapositive, constructive de Morgan,
  and the `¬∃x.P ↔ ∀x.¬P` quantifier duality — with their classical converses (excluded
  middle, `¬¬A→A`, `¬∀→∃¬`) pinned as **rejected**, marking the logic intuitionistic.
- **order theory** (`delta/ORDER.md`): `≤` and `<` defined purely in the logic
  (`a≤b := ∃c. a+c=b`, `a<b := ∃c. a+Sc=b`) and proved a **total order** and a **strict
  order** respectively — reflexivity, transitivity, antisymmetry, totality, trichotomy
  (`a<b ∨ a=b ∨ b<a`), the mixed transitivities, and monotonicity under `+` and `·` — so
  the naturals are a **linearly ordered commutative semiring**; plus number theory built on
  the same machinery: every number is **even or odd**, the **discreteness** of `<`
  (`a<b ↔ Sa≤b`), positivity (`0<a ∧ 0<b → 0<a·b`), **divisibility** (`a∣b := ∃m. m·a=b`)
  proved a **partial order** (reflexive, transitive, antisymmetric `a∣b ∧ b∣a → a=b` — via
  `1` being the only unit, `d∣1 → d=1`), the list analog of `0`-or-`s`,
  `∀l. l=nil ∨ ∃h∃t. l=cons h t`, and — the capstone — the **Euclidean division algorithm**
  `∀b. 0<b → ∀a. ∃q∃r. a=b·q+r ∧ r<b`, proved **existence** (by a fuel induction with a
  strict-trichotomy case split, since the first-order checker can't instantiate strong
  induction at a concrete predicate) **and uniqueness** (`q` and `r` are determined — a
  bounding argument resting on **multiplication being cancellative**, `0<b ∧ b·q₁=b·q₂ → q₁=q₂`);
  **GCD existence**, `∀a∀b. ∃g. g∣a ∧ g∣b ∧ ∀d.(d∣a ∧ d∣b → d∣g)` (a greatest common divisor
  every common divisor divides), constructed by the **Euclidean algorithm** as a fuel induction
  on top of division and the `divides-add`/`divides-sub` closure laws — 93 lemmas in one proof;
  **decidable divisibility** `0<d → (d∣n ∨ ¬(d∣n))` (constructive, from division existence +
  uniqueness — no excluded middle); and — the deepest result — **prime-divisor existence**,
  `∀n>1. ∃p. prime(p) ∧ p∣n` (every number above 1 has a prime factor — the gateway to the
  fundamental theorem of arithmetic), 175 lemmas, by a strong/fuel induction over a bounded
  divisor search with a primality criterion, all resting on decidable divisibility; and — the
  capstone — **the fundamental theorem of arithmetic** (existence half),
  `∀n. 0<n → ∃L. ProdIs(L,n) ∧ (∀x. x∈L → prime(x))` — **every positive integer is a product
  of a list of primes** — 231 lemmas in one certificate, by a fuel induction that factors `n`
  as `p·m` (prime `p` from prime-divisor existence, `m<n`) and prepends `p` to a factor list of
  `m`. Stating it required two new **inductive predicates** in the checker's core (the first
  logical primitives added since induction itself): list **membership** `Mem(x,L)` and the
  relational product **`ProdIs(L,n)`**, each defined by introduction rules with matching
  sound inversions and mirrored identically across all three checkers. "Every element is
  prime" — unstatable in the bare first-order logic — is now `∀x. Mem(x,L) → prime(x)`.
  And — the other deep classical pillar — **Euclid's lemma**,
  `prime(p) ∧ p∣(a·b) → p∣a ∨ p∣b` (194 lemmas), proved the **Bézout-free / ℤ-free** way by the
  smallest-multiple argument (the least `m>0` with `p∣mb` divides every such `k`, so `m∣p`,`m∣a`,
  and primality forces `m=1`→`p∣b` or `m=p`→`p∣a`). From it follows a strong **uniqueness**
  half of the FTA: every prime dividing `n` appears in *any* prime factorization
  (`prime p ∧ ProdIs(L,n) ∧ p∣n ∧ ∀x∈L.prime x → Mem(p,L)`), so the *set* of primes is
  determined; the empty product is uniquely the empty list; and a prime factors uniquely as the
  singleton `[p]`. And — the second of the two classical crown jewels — **the infinitude of
  primes** (Euclid, 244 lemmas): `∀L. allPrime(L) → ∃q. prime(q) ∧ ¬Mem(q,L)` — *any* finite list
  of primes omits one, proved constructively by `N = product(L)+1` (which is `>1` since a product
  of primes is positive), whose prime factor cannot lie in `L` (it would divide both `product(L)`
  and `N`, hence `1`). (Independently, a complete **ℤ** is constructed as difference pairs and
  proved a **linearly ordered commutative ring** up to `~`, with the ℕ-embedding an
  order-isomorphism — see [`delta/INTEGERS.md`](delta/INTEGERS.md).)
- **exponentiation** is a recursive function over a *user*-nat exponent (no kernel change), proved
  to satisfy the three classical power laws — `(a·b)^k = a^k·b^k`, `a^(m+n) = a^m·a^n`, and
  `(a^m)^n = a^(m·n)` — and to **respect congruence** (`a ≡ b (mod n) → a^k ≡ b^k (mod n)`, the
  basis of modular exponentiation), tying the exponentiation chapter back to the ℤ/nℤ ring.

### Why you can believe it

The trust anchor is defended five independent ways (all under `verify-lattice.sh`):

- **331-case gate** (`test.sh`) — valid certificates accepted, invalid rejected.
- **43-case soundness battery** (`soundness.sh`) — invalid certificates that must
  *all* be rejected, including classical-but-non-constructive tautologies (excluded
  middle, Peirce, the drinker paradox) and fabricated memberships/products.
- **83-case checker diamond** (`checker-diamond.sh`) — *diversity = security* applied
  to the checker itself: `check.beta` (Beta, tagged-memory + CFG guard-state dispatch) and
  `gamma/checker.gamma` (Gamma, ADTs + pattern matching) must return identical
  verdicts on every proof. It has caught real divergences.
- **type-safety** — `gamma/checker_typed.gamma` is the checker fully annotated, and
  gamma's own type checker accepts it: the trust anchor's *code* is statically safe.
- **soundness seams** — `semantics-diamond.sh` (definitional `=` vs the interpreter's
  operational eval) and `induction-soundness.sh` (inductively-proved universals
  confirmed against the interpreter at concrete instances). These are *evidence* for
  the open soundness theorem `provable ⟹ true-about-the-reference-interpreter`, not a
  proof of it.

## Honest frontiers

- The soundness theorem itself is the deep open problem.
- **User-defined recursive functions** over user types are implemented — arity 0/1/2
  constructors *and* 1/2 arguments (a binary `Tree` fold; binary user-`add`), reductions
  feeding the conversion rule. *Theorems* over user functions prove by induction, not
  just induction principles: `∀ user-n. g(n)=h(n)` and the canonical `∀x. add(x,Z)=x`
  (right identity — the user-function analogue of `n+0=n`). Verified every way the rest
  of the anchor is: all three checkers (gate, soundness, **checker diamond**, type-safety)
  plus eq.beta and the **semantics diamond**. See [`delta/FUNCTIONS.md`](delta/FUNCTIONS.md).
  A number type with user-defined `add`/`mult` is even shown to be a full **commutative
  semiring** (`add` a commutative monoid; `mult` distributes over `add` *and* commutes),
  inside the checker, by induction + the lemma layer — the same axioms built-in `+`/`*`
  meet. The `mult`-commutativity proof is the deepest: it rests on a left-expansion lemma
  and must respect the checker's eigenvariable condition on `gen`. **Folds compose with the
  built-in arithmetic**: a user list-of-naturals carries a `product` and an `append`, and
  `product` is proved a **monoid homomorphism** `product(l₁++l₂) = product(l₁)·product(l₂)`
  (gate "product homomorphism") by structural `rec` induction — the expressible core of
  factorization. (A *former* frontier — "the FTA needs recursive **propositions** the checker
  lacks, so 'every element of the list is prime' can't even be stated" — has since been **crossed**:
  two inductive predicates (`Mem`, `ProdIs`) were added to the kernel core, and the FTA existence
  half, Euclid's lemma, the uniqueness pieces, and the infinitude of primes are now all proved; see
  the trust-anchor section above.) Remaining genuine frontiers here: N-ary (3+) `fun` arguments,
  and `fun` rules still pattern-match only *user* constructors (no Bool test over built-in naturals).
- epsilon (systems language) and omega (full dependent types) are design-stage.
