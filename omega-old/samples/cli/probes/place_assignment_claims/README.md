# place_assignment_claims

First sample assigning a claim-bearing call result into an
existing place rather than a `let` binding: `pair.left =
make(5)` continues `make`'s claim at the destination member
after the old receipt was moved out — no live obligation is
overwritten.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
