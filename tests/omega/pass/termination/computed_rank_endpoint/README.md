# computed_rank_endpoint

A rank endpoint may be computed from another parameter:
`in 0..(cap / 5 + 6)` — an exclusive-open arithmetic endpoint,
strengthened by `requires 0 <= cap <= 20`; the iterate state's own
requires repins both bands.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
