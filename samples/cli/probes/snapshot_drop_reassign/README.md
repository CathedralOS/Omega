# snapshot_drop_reassign

Scalar snapshot copied out before `drop(g)` stays usable; whole
`g = Guard { handle: snapshot }` re-establishes the owner and later reads
observe the new value; then `consume(g)` transfers.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
