# requires_interval_construct

First sample with prover-backed construction: the machine's
`requires 1 <= strength, strength <= 100` interval decides
the gated `data Player requires health >= 1` construction
`Player { health: strength }` — no spelled proof needed at
the literal.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
