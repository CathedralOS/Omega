# Order theory, and a first-order completeness gap in the checker

## The order relation

Order on the naturals needs **no new checker machinery** — `≤` is definable in the
existing first-order logic with `∃` and `+`:

```
a ≤ b  :=  ∃c. a + c = b
```

The **∃-introduction** fragment already proves the foundational facts (all pinned in the
gate, `test.sh`):

| theorem | witness | justification |
| --- | --- | --- |
| `∀a. a ≤ a` (reflexive) | `c = 0` | `a+0=a` (the `n+0=n` lemma) |
| `∀a. 0 ≤ a` (least element) | `c = a` | `0+a=a` definitionally |
| `∀a∀b. a ≤ a+b` | `c = b` | `a+b=a+b` by `refl` |

These establish `≤` as a reflexive relation with `0` least and compatible with `+` on the
right — the start of an ordered-semiring backbone on top of the commutative semiring.

## The gap: ∃-elimination under an open hypothesis

**Transitivity** `∀a∀b∀c. a≤b → b≤c → a≤c` needs **∃-elimination** (`unpack`): open
`h1 : ∃d. a+d=b` and `h2 : ∃e. b+e=c`, then witness `f = d+e` with
`a+(d+e) = (a+d)+e = b+e = c`. The proof is mathematically routine, but the checker
**rejects** it — and the minimal reproduction shows why:

```
; ACCEPTS — unpack under a CLOSED hypothesis
(-> (Exists (= (p z (v 0)) z)) (Exists (= (p z (v 0)) z)))
  (lam … (unpack (hyp 0) (gen (lam … (wit … (v 0) (hyp 0))))))

; REJECTS — same shape, but the hypothesis mentions an outer ∀-bound variable a
(All (-> (Exists (= (p (v 1) (v 0)) (v 1))) (Exists (= (p (v 1) (v 0)) (v 1)))))
  (gen (lam … (unpack (hyp 0) (gen (lam … (wit … (v 0) (hyp 0)))))))
```

`unpack`'s handler is a `(gen (lam …))`, and `gen` runs an **eigenvariable check**
(`check.beta`, the `gen_body` state):

```
to reject when (free_ivp(word[4194304 + t1 * 8], 0) == 1)   ; no free indiv var in scope
```

It rejects the generalization if **any** hypothesis in the context mentions **any**
individual variable. That is *sound but incomplete*: it conflates "the hypothesis depends
on the variable being generalized" (the real eigenvariable condition) with "the hypothesis
mentions any variable at all". Because `gen` does **not shift** the stored hypothesis
props when it enters a new individual binder, it cannot tell the two apart, so it
conservatively forbids the whole situation.

## The fix (DONE, soundness-critical)

The principled cure is the standard de Bruijn move: when `gen` enters a binder, the
context's hypotheses are one binder deeper, so their individual-variable indices must
**shift up by 1** (cutoff 0). It is implemented across all three checkers:

- **`check.beta`** — an individual-binder-depth counter `IDEP` (memory `2097112`) alongside
  `CTXN`; `ctx_push` records `IDEP` per hypothesis (parallel array at `4718592`); `gen`
  does `IDEP++ / infer / IDEP--`; and `hyp` lookup returns `shift_prop(stored, IDEP −
  stored_idep, 0)`. The old conservative `free_ivp` reject in `gen` is gone.
- **`gamma/checker.gamma`** and **`gamma/checker_typed.gamma`** — purely functional:
  `(Gen pf) → All (infer pf (shiftctx ctx))`, where `shiftctx` maps `shiftp · 1 0` over the
  context. `ctxclean` is no longer consulted.

After the shift, no hypothesis can reference the freshly bound variable (`Iv 0`) — each was
pushed at a shallower depth — so the eigenvariable condition holds **structurally**.

Why this stays sound: in this checker you never "generalize an existing variable", you
only introduce a **fresh** one via `gen`. A hypothesis like `x = 0` (about an outer `x`)
does not block `gen`, because `gen` binds a *new* variable `y ≠ x`; the proof obtained is
`∀y. (x=0 → …)`, never `∀x. x=0`. The de Bruijn structure already prevents the unsound
conflation; correct shifting just lets the checker *see* that.

**Verified** against the full battery (`verify-lattice.sh`): every prior accept/reject
unchanged; the soundness suite still rejects every bad certificate, now including the
minimal adversarial pair `P(x) → ∀y.P(y)` and `x=0 → ∀x.x=0` (both **rejected**), while the
sound `P(x) → ∀y.P(x)` is accepted; both checkers agree on an open-hypothesis `unpack` and
a `gen`-capture in the checker diamond.

## Order theory unlocked by the fix

With `∃`-elimination under open hypotheses, `≤` is proved a **partial order**, all pinned
in the gate:

- `le trans` — `∀a∀b∀c. a≤b → b≤c → a≤c` (witness `d+e`, via associativity);
- `le antisym` — `∀a∀b. a≤b → b≤a → a=b` (unpack both witnesses `c,d`; then
  `a+(c+d) = (a+c)+d = b+d = a = a+0`, so cancellation gives `c+d=0`, sum-zero gives
  `c=0`, and `a+c=b` collapses to `a=b`).

- `le total` — `∀a∀b. a≤b ∨ b≤a`. With the partial-order laws this makes `≤` a **total
  (linear) order**. The largest proof in the project: induction on `a`, the step cases on
  `b` (0 or successor) and in the successor case `b=Sm` applies the IH at `m` and lifts it
  with `le-succ-mono`, transporting `Sm=b` through the witness equation — `case → ∃-elim →
  case → ∃-elim`, five binder levels deep.

Supporting backbones, also enabled by the fix (their induction step generalizes under a
hypothesis that mentions the induction variable): **additive left-cancellation**
`∀a∀x∀y. a+x=a+y → x=y` (`add cancel`, via S-injectivity), **sum-zero** `∀x∀y. x+y=0 → x=0`
(`sum zero`, via Peano no-confusion), monotonicity (`le succ mono/refl`, `le zero`), and
the arithmetic lemmas `right succ` (`a+Sm=S(a+m)`) and `le succ self` (`a≤Sa`).

**Multiplication monotonicity** `mult mono` — `∀a∀b∀c. a≤b → a·c≤b·c` (witness `k·c`, via
right-distributivity) — makes the order compatible with `·` as well as `+`, so the naturals
are an **ordered commutative semiring**.

## Strict order

`a < b := ∃c. a+Sc=b` (constructive: `b` exceeds `a` by at least one). Pinned in the gate:

- `lt to le` — `a<b → a≤b` (re-witness with `Sc`);
- `zero lt S`, `lt succ self` — `0<Sa`, `a<Sa` (witness `0`);
- `lt irrefl` — `¬(a<a)`: `a+Sc=a` forces `Sc=0` by cancellation, refuted by no-confusion;
- `lt trans` — `a<b → b<c → a<c` (witness `k+Sj`, via associativity);
- `lt to succ le` — `a<b → Sa≤b`, the successor bridge between `<` and `≤`.

- `lt asym` — `a<b → ¬(b<a)`: `a<b` and `b<a` give `a<a` by transitivity, refuted by
  irreflexivity. So `<` is **irreflexive, transitive, and asymmetric** — a strict order in full.
- `le split` — `a≤b → (a<b ∨ a=b)`: unpack the `≤` witness and case on 0-or-successor.
- `trichotomy` — `∀a∀b. a<b ∨ a=b ∨ b<a`, the **defining law of a linear order**: from
  totality (`a≤b ∨ b≤a`) and `le-split` applied to each branch.

The naturals are now a fully **linearly ordered commutative semiring** inside the trust
anchor: `≤` a total order compatible with `+` and `·`, `<` a strict order beneath it
(also `+`-monotone, `lt add mono`), the `Sa≤b` bridge between them, and trichotomy tying
it all together.

## The deep capstone: strong induction (well-foundedness of `<`)

`strong induction` — for an **abstract predicate** `P = Pred 0`:

```
(∀n. (∀m. m<n → P m) → P n)  →  ∀n. P n
```

This is the well-founded-induction principle for `<` — not a fixed arithmetic fact but a
*proof scheme* over an uninterpreted predicate, the strongest statement the first-order
checker can make about the order. It is proved by ordinary `natind` on the auxiliary
`Q(n) := ∀m. m<n → P m`:

- **base** `Q(0)` is vacuous — `lt not zero` (`¬(m<0)`) discharges the antecedent by `absurd`;
- **step** `Q(n') → Q(Sn')`: given `m<Sn'`, the bridge `lt S to le` gives `m≤n'`, then
  `le split` gives `m<n' ∨ m=n'` — the first case uses the induction hypothesis `Q(n')`,
  the second rewrites `m=n'` and applies the supplied hypothesis at `n'`.

Then `P n = H(n)(Q(n))` for every `n`. A mismatched-predicate variant is **rejected**,
confirming the checker genuinely checks it. This result leans on the whole stack — the
eigenvariable fix (so `∃`-elimination and the abstract hypothesis thread through the nested
induction), the order lemmas, and `le-split` — and is the natural summit of the order
backbone.
