# hold_state_call

A member may issue a component call from a subordinate state:
`hold -> step(pending)` transports `pending` to the callee's
discovered entry role `n`, so the edge is weak (`remaining`
unchanged) while the strict `step -> count(n - 1)` edge completes
every cycle's descent.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
