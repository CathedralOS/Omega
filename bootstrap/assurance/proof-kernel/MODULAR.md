# Modular arithmetic in the proof kernel (ℤ/nℤ)

Congruence modulo `n` — `a ≡ b (mod n)` — defined purely in the first-order logic over the
naturals, *without* subtraction, by the difference-pair trick (the same one that builds ℤ, see
[`INTEGERS.md`](INTEGERS.md)):

```
a ≡ b (mod n)  :=  ∃qa ∃qb.  a + n·qb = n·qa + b
```

This is exactly `n | (a − b)` read in ℤ: `a − b = n·(qa − qb)` becomes the subtraction-free
`a + n·qb = n·qa + b`. The two witnesses `qa, qb` stand in for the (possibly negative) quotient.

## Proved (gate, `corpus/proofs/mod-*.elab`)

- **`≡` is an equivalence relation**: **reflexive** (`qa=qb=0`, reducing to `a+0 = 0+a`),
  **symmetric** (swap the two witnesses + commutativity), and **transitive** (witnesses add:
  `qe=qa+qc`, `qf=qb+qd`; the body chains through distributivity `n·(x+y)=n·x+n·y` and
  associativity, with the shared `b` cancelling between the two hypotheses).

- **`≡` is a congruence** for `+` and `·` (single-sided: `a≡b → a+c ≡ b+c` and `a·c ≡ b·c`; the
  multiplicative case multiplies the hypothesis through by `c` via right-distributivity and
  reassociates `(n·q)·c = n·(q·c)`). So **ℤ/nℤ is a well-defined commutative ring**.

## Next

Two-sided compatibility (`a≡a' ∧ b≡b' → a+b ≡ a'+b'`, immediate from the single-sided ones + the
equivalence), then Fermat's little theorem / Euler's theorem as the deep targets (these would
also exercise the prime theory already in the gate).
