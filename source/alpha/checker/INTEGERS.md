# Integers in the proof kernel (the ℤ layer)

The checker decides first-order logic over the built-in **naturals** — there is no
subtraction, so `∀a∀b. ∃c. a = b + c` is false and Bézout's `gcd = ax − by` is not even
expressible. To reach Euclid's lemma, Bézout, and the *uniqueness* half of the fundamental
theorem of arithmetic, the naturals must be extended to **ℤ**.

## Construction: difference pairs (the Grothendieck completion)

An integer is a **pair of naturals** `(a, b)`, read as the difference `a − b`. No new data
type or kernel rule is needed — a pair is just two `ℕ` components, and every integer operation
is *pure `ℕ` arithmetic on the components* (crucially, the **definitions use no subtraction**):

| ℤ notion        | on pairs `(a,b)`, `(c,d)`                    |
| --------------- | -------------------------------------------- |
| equality `~`    | `intEq((a,b),(c,d)) := a + d = c + b`        |
| zero / one      | `(0,0)` / `(1,0)`                            |
| addition `+`    | `(a+c, b+d)`                                 |
| negation `−`    | `(b, a)`                                     |
| multiplication  | `(a·c + b·d, a·d + b·c)`                     |

Because the representation is not canonical (`(1,0) ~ (2,1) ~ …`), `~` is a **setoid
equivalence**, not definitional equality — so every law is stated *up to `~`*, and the first
job is to prove `~` *is* an equivalence and that the operations respect it.

## Proved so far (all in the gate, `corpus/proofs/int-*.proof`)

- **`~` is an equivalence relation** — reflexive, symmetric, **transitive** (transitivity is
  the substantive one: it rests on right-cancellation of `ℕ` addition plus a 4-term reordering).
- **`(ℤ, +, 0, −)` is an abelian group up to `~`** — addition **commutes**, is **associative**,
  has `(0,0)` as **identity**, and — the property ℕ structurally lacks — **every integer has an
  additive inverse**, `x + (−x) ~ 0` (which reduces to `ℕ` commutativity `a + b = b + a`).
- **`(ℤ, +, ·, 0, 1)` is a commutative ring up to `~`** — multiplication **commutes**, has
  `(1,0)` as **identity**, and **distributes** over addition, `x·(y+z) ~ x·y + x·z` (the deep
  axiom: both sides expand to the same eight atomic products, reconciled by an `add4`
  reordering helper `(w+x)+(y+z) = (w+y)+(x+z)`).
- **`~` is a congruence** — `+`, `−`, and `·` are **well-defined on `~`-classes**:
  `x~x' → x+y ~ x'+y`, `−x ~ −x'`, and `x·y ~ x'·y`. So ℤ = pairs/`~` is a *well-defined*
  commutative ring (the multiplicative case multiplies the hypothesis through each column via
  right-distributivity). This is the prerequisite for any quotient/substitution reasoning.
- **a linear order `x ≤ y := ∃k∈ℕ. (a+d)+k = (c+b)`** (i.e. `a−b ≤ c−d ⟺ a+d ≤ c+b` in ℕ) —
  **reflexive**, **antisymmetric** (`x≤y ∧ y≤x → x~y`), **transitive**, and **total**. The key
  move is that `intLe(x,y)` *is* the ℕ relation `NLe(a+d, c+b)`, so transitivity reduces to
  ℕ add-monotonicity + ℕ le-transitivity + cancellation, and totality is immediate from ℕ
  totality. The embedding `ι(n) = (n,0)` is an **order-isomorphism onto its image** (`m≤n ⟺
  ι(m)≤ι(n)`) and **injective** up to `~`. So **ℤ is a linearly ordered commutative ring** up to `~`.

## Next

The road to **FTA uniqueness**: `ℤ`-division-with-remainder (or Bézout via the least positive
value of `{ax+by}` using ℕ well-ordering), then **Bézout** (`gcd = ax+by`, finally expressible),
then **Euclid's lemma** (`prime p ∧ p∣ab → p∣a ∨ p∣b`), then **FTA uniqueness** — closing the
theorem whose existence half is already in the gate. (Multiplication associativity remains a
deferred ring-completeness item — an 8-way triple-product expansion not on this critical path.)
