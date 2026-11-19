# guarded_call_requires

First sample discharging a callee `requires` from a
dominating incoming-arm guard:
`transition main.a <= main.b { true -> go }` puts the fact
on every edge into `go`, proving `helper`'s
`requires main.a <= main.b` (a carrier-field read) and the
dependent argument; inside `helper` the requires proves the
exact subtraction. Unguarded callers refuse (fail sibling).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
