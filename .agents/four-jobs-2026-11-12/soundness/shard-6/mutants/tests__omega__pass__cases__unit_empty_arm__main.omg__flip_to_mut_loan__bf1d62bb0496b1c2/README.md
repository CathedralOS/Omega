# unit_empty_arm

`true -> {}` — the conditional unit completion: a transition arm whose
empty body ends the machine without a successor state, beside a sibling
arm that forwards to a writer state.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
