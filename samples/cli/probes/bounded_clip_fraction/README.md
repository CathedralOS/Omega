# Bounded clip fraction

A Sutherland–Hodgman edge crosses a constraint when the two endpoint
distances have opposite signs. Their difference is the denominator of the
intersection fraction. This program isolates that denominator: a computed
`f64` difference, a guard that it lies in `[0.5, 8]`, and the finite quotient
of a numerator in `[0, 4]`.

Three spellings share that guard discharge. The literal may sit on either
side of the comparison, and the compared place may be a field. Each
quotient of `1 / 2` is `0.5`. The destination parameter carries the proved
range; a later local initializer does not inherit the guard by itself.

```sh
omega run samples/cli/independent/bounded_clip_fraction/main.omg
```

Expected exit: 0.

`invalid/` guards only the lower bound and must still reject: `difference >= 0.5`
does not prove `difference <= 8`.
