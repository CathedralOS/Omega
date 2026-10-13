# nonreturning_unit

A Unit result promises the SHAPE of a return, not that a return occurs: `wait_forever` is a valid Unit machine whose states only self-transition.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
