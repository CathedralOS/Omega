# invariant_window_states

First sample with a standing invariant allowed to break
mid-state: `data Main requires start <= end` — `start = 10`
violates it transiently, `end = 20` restores it before the
next transition; the bound is checked at state boundaries,
not between statements.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
