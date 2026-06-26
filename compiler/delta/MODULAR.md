# Modular arithmetic in the delta checker (ℤ/nℤ)

Congruence modulo `n` — `a ≡ b (mod n)` — defined purely in the first-order logic over the
naturals, *without* subtraction, by the difference-pair trick (the same one that builds ℤ, see
[`INTEGERS.md`](INTEGERS.md)):

```
a ≡ b (mod n)  :=  ∃qa ∃qb.  a + n·qb = n·qa + b
```

This is exactly `n | (a − b)` read in ℤ: `a − b = n·(qa − qb)` becomes the subtraction-free
`a + n·qb = n·qa + b`. The two witnesses `qa, qb` stand in for the (possibly negative) quotient.

## Proved (gate, `proofs/mod-*.elab`)

- **`≡` is an equivalence relation**: **reflexive** (`qa=qb=0`, reducing to `a+0 = 0+a`),
  **symmetric** (swap the two witnesses + commutativity), and **transitive** (witnesses add:
  `qe=qa+qc`, `qf=qb+qd`; the body chains through distributivity `n·(x+y)=n·x+n·y` and
  associativity, with the shared `b` cancelling between the two hypotheses).

## Next

`≡` is a **congruence** for `+` and `·` (`a≡a' ∧ b≡b' → a+b ≡ a'+b'` and `a·b ≡ a'·b'`), making
**ℤ/nℤ a well-defined commutative ring** — then Fermat's little theorem / Euler's theorem as the
deep targets (these would also exercise the prime theory already in the gate).
