# ensures_tail_forward

`machine forward() -> i32 ensures result == 70 { transition { _ ->
answer() } }` — a bare tail-call transition arm discharges the caller's
`ensures` by forwarding the callee's own `ensures result == 70` along
the call edge.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
