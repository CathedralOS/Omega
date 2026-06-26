# Integers in the delta checker (the ℤ rung)

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

## Proved so far (all in the gate, `proofs/int-*.elab`)

- **`~` is an equivalence relation** — reflexive, symmetric, **transitive** (transitivity is
  the substantive one: it rests on right-cancellation of `ℕ` addition plus a 4-term reordering).
- **`(ℤ, +, 0, −)` is an abelian group up to `~`** — addition **commutes**, is **associative**,
  has `(0,0)` as **identity**, and — the property ℕ structurally lacks — **every integer has an
  additive inverse**, `x + (−x) ~ 0` (which reduces to `ℕ` commutativity `a + b = b + a`).

## Next

Multiplication respects `~` and the ring axioms (distributivity, `ℤ` a commutative ring);
then the order, then the extended Euclidean algorithm / Bézout, then Euclid's lemma, then FTA
uniqueness.
