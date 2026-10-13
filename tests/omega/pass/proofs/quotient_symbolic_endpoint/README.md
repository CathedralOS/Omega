# quotient_symbolic_endpoint

A rank endpoint may divide by a second symbolic parameter:
`in 0..(cap / divisor + 6)` — three-parameter symbolic quotient,
strengthened by the requires band `1 <= divisor <= 5`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
