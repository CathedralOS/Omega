# The road to FTA *uniqueness* (the delta corpus's headline frontier)

The **existence** half of the fundamental theorem of arithmetic is in the gate
(`proofs/fundamental-theorem-arithmetic.elab`: every `n>0` is a product of a list of primes).
The **uniqueness** half — *that factorization is unique up to order* — is the corpus's remaining
headline target. This note inventories what is already proved, identifies the single missing
construct, and specifies the plan, so the (careful, trust-anchor-touching) work can be picked up
cleanly across iterations.

## Stating it

With `ProdIs(L,n)` ("the product of list `L` is `n`"), `allPrime(L) := ∀x. Mem(x,L) → prime(x)`,
and a list-permutation relation `Perm`:

> **FTA uniqueness.**  `ProdIs(L1,n) & ProdIs(L2,n) & allPrime(L1) & allPrime(L2) → Perm(L1,L2)`.

## What is already proved (all in `proofs/`, all checked by the seed-rooted anchor)

The *pure* lemmas the uniqueness induction needs are **all present**:

- `prime-divides-prime` — `prime(p) & prime(q) & p|q → p=q`. The equality step: a prime that divides
  a prime *is* it. (The newest building block.)
- `prime-factor-in-list` — `prime(p) & ProdIs(L,n) & p|n & allPrime(L) → Mem(p,L)`. Euclid's lemma
  lifted to a list: a prime dividing a prime-product is one of the primes.
- `member-divides-product` — `Mem(x,L) & ProdIs(L,n) → x|n`. The converse direction.
- `product-one-list-nil` — `ProdIs(L,1) & allPrime(L) → L=nil`. The induction's **base case**.
- `prime-factorization-singleton` — `prime(p) & ProdIs(L,p) & allPrime(L) → L = cons p nil`.
  Uniqueness already holds **for a prime `n`** (the singleton case), via exactly the pinch below.
- `mult-cancel` / `mult-cancel-right` — `a*c = b*c & 0<c → a=b`. Cancels the shared prime.
- `cons-injective`, `len-zero-nil` — list structural facts.
- `prime-divisor-existence`, `euclid-lemma`, `gcd-existence/uniqueness` — the number-theory base.

So the *arithmetic* is finished. `prime-factorization-singleton`'s proof is the whole argument in
miniature (cons `h t`: `h|p` so `h=p` by `prime-divides-prime`; `p=p*m` so `m=1` by `mult-cancel`;
`ProdIs(t,1)` so `t=nil` by `product-one-list-nil`). General uniqueness is that argument under an
induction that must *track positions* — which is what a permutation relation is for.

## The one missing construct: a general permutation relation

The only `Perm` in the corpus is `perm-sum` — a **2-element** notion (`{x,y}={a,b} → x+y=a+b`),
not a relation on arbitrary lists. A multiset/permutation relation on lists cannot be *defined* as a
formula over the existing primitives: a `Count(x,L)` function would need a conditional on nat-equality,
which the checker's term language (`z s p m nil cons app len` + user constructors) cannot express, and
set-equality `∀x. Mem(x,L1)↔Mem(x,L2)` is **wrong** (it loses multiplicity: `12=2·2·3`).

So `Perm` must be added as an **inductive predicate in the checker core**, exactly as `Mem` (rel 777)
and `ProdIs` (rel 778) already are. The standard 4-rule presentation (the multiset congruence on lists):

| rule        | conclusion                              | premise                |
| ----------- | --------------------------------------- | ---------------------- |
| `perm-nil`  | `Perm(nil, nil)`                        | —                      |
| `perm-skip` | `Perm(cons x t1, cons x t2)`            | `Perm(t1, t2)`         |
| `perm-swap` | `Perm(cons x (cons y r), cons y (cons x r))` | —                 |
| `perm-trans`| `Perm(a, c)`                            | `Perm(a,b) & Perm(b,c)`|

## Checker-change checklist (this is a TRUST-ANCHOR edit — do it carefully, with full budget)

1. **`check.beta`** — ✅ DONE. `Perm = Rel 779`, four intro rules at alloc tags 57–60 (`permnil`,
   `permskip x pf`, `permswap x y r`, `permtrans pf1 pf2`). Parse: a `kc2=='e'` branch under `is_p`
   (dispatched by `kc5`/`kc6` to nil/skip/swap/trans); check: `d57`–`d60` mirroring the `mem`/`prod`
   rules (`permtrans` matches the shared middle term with `conv_eq`). Intro-only ⇒ sound, no inversion.
   Gotcha banked: the list argument of `permswap` is an individual/list **term**, so it is parsed with
   `parse_nat` (the misleadingly-named general term parser, handles `nil`/`cons`), NOT `parse_term`
   (which is the proof parser and starts with `expect('(')`). Validated with 5 positive + 3 negative
   raw certs; `elab.py` got the four matching keyword cases. `proofs/perm-refl.elab` lands (∀L. Perm(L,L)
   by `listind`: nil→permnil, cons→permskip on the IH). elab-test now **206 ok**; full lattice VERIFIED.
2. **`checker.gamma`** (+ `checker_typed.gamma`) — ⏳ NEXT (Stage 2). Mirror the four rules in the
   pattern-match style (see the `MemHead`/`Pcons` arms ~line 358): `((Permnil) (Rel 779 Lnil Lnil))`,
   `((Permskip x pf) …(Rel 779 (Lcons x t1) (Lcons x t2))…)`, `((Permswap x y r) (Rel 779 (Lcons x
   (Lcons y r)) (Lcons y (Lcons x r))))`, `((Permtrans p1 p2) …conv_eq the middle…)`. Add ADT variants
   in `checker_typed.gamma` (erase_types.py derives the untyped 3rd oracle). **`checker-diamond.sh`
   requires all checkers to agree**, so add ~6 `dia()` cases (both syntaxes) when the mirror lands.
   Until then Perm is check.beta-only: the existing 205 stay fully 3-checker-gated; perm proofs are
   check.beta-gated (via elab-test) — a documented, temporary asymmetry.
3. **Re-verify** `elab-test.sh`, `checker-diamond.sh`, `semantics-diamond.sh`, full `verify-lattice.sh`.
4. Then `perm-sym` (skip/swap/trans induction) and the **member-to-front surgery** lemma below.

## Uniqueness proof plan (once `Perm` exists)

Strong induction on `n`:
- `n=1`: `ProdIs(L1,1)` and `ProdIs(L2,1)` give `L1=L2=nil` (`product-one-list-nil`); `Perm(nil,nil)`.
- `n>1`: write `L1 = cons p t1` (`p` prime; `p|n` since `n=p·∏t1`). `prime-factor-in-list` ⇒ `Mem(p,L2)`.
  A **member-to-front surgery** `Mem(p,L2) → ∃L2'. Perm(L2, cons p L2') & ProdIs(L2', m)` (the new
  `Perm`-dependent lemma to prove) plus `mult-cancel` give `ProdIs(t1,m)` and `ProdIs(L2',m)` with
  `m=n/p < n`. The IH yields `Perm(t1,L2')`; `perm-skip` lifts it to `Perm(cons p t1, cons p L2')`;
  `perm-trans` with `Perm(cons p L2', L2)` (from the surgery, symmetrized) closes `Perm(L1,L2)`.

This is the lattice's deepest pure-math capstone; it is a multi-iteration effort whose first move is
the checker-core `Perm` predicate above.
