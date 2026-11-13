# erased_state_param

`state store(n: i32, bound [erased]: i32) requires n < bound` — an
`[erased]` machine parameter flows into an `[erased]` state parameter
through a named transition `store(n, bound)`; the state's own requires
re-cites it.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
