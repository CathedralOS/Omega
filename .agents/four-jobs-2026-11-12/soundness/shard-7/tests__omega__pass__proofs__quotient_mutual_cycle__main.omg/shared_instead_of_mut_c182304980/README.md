# quotient_mutual_cycle

A mutual call cycle where BOTH members declare quotient rank
endpoints: `first` ranks `remaining in 0..(cap / 5 + 6)` and
`second` ranks `pending in 0..(limit / 5 + 6)` — each edge's
delivered coordinate re-instantiates the callee's endpoint.

## Learnings

- `Main::main` (or any caller state) that makes a terminal machine
  call must declare the same result type — a terminal call's normal
  result is its caller's result.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
