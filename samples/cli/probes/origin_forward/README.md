# origin_forward

First sample proving `let` preserves entry origin:
`held_local(input) ensures result == input` holds through a
forwarded immutable local — and through a chained `a -> b`
copy — because custody keeps the exact incoming identity.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
