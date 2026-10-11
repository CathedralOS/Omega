# guard_compound_index

A compound index bound follows an explicit data subject into its
successor: the edge guard `main.b * 20 + main.a >= 0 && ... < 200`
discharges the `grid` write in `put` — the same compound expression
serves as guard fact and index. The array lives wholly
`in Wrapping`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
