# utf8_arrival

First sample minting a byte predicate at a transition:
`transition valid_utf8(view)` produces the fact the target
state's `requires valid_utf8(view)` discharges as a block
invariant on that arriving edge (unguarded arrival
rejects — the fail sibling covers it).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
