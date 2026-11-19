# callee_establishes

First sample proving establishment is globally monotone:
`recruit -> Player` returns a gated record — the call's
result joins the caller's tracking so the read AFTER the
call passes the access gate (gated records can't be carrier
fields: the entry context must be valid at zero).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
