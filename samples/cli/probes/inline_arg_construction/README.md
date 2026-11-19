# inline_arg_construction

First sample with claim-bearing constructions inline at call
arguments — `settle(Pair { left: moved, right: fresh })` and a
nested `Envelope { pair: Pair { ... } }`. An anonymous
construction is a fresh owner; a moved member's claim
continues into its field position.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
