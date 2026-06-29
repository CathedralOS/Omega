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

1. **`check.beta`** — add four alloc tags (e.g. 44–47) mirroring the `memhead`/`memtail` pattern:
   a parse state per rule (see `is_m → do_memhead`, alloc tag 42; `do_memtail`, tag 43; `is_pinv` for
   ProdIs inversion), and a check rule per tag in the `d42`/`d43`/… dispatch that verifies the premise
   sub-proofs and emits the `Rel <permId> a b` proposition.
2. **`checker.gamma`** — mirror the same four rules. **The `checker-diamond.sh` gate requires
   `check.beta` and `checker.gamma` to agree on every certificate**, so they must move together.
3. **Re-verify** `elab-test.sh` (existing 205 proofs must still pass — `Perm` is new, so they are
   unaffected), `checker-diamond.sh`, `semantics-diamond.sh`, and the full `verify-lattice.sh`.
4. Add a first `Perm` lemma to exercise it (`perm-refl` via `perm-skip` list-induction, then `perm-sym`).

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
