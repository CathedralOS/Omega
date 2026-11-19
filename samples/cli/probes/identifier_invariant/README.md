# identifier_invariant

`invariant` is not reserved syntax — it binds as an
ordinary local name (`let invariant: i32 = 25`).
Semantic invariant windows and loop reasoning stay
independent mechanisms.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
