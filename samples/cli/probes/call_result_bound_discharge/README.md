# call_result_bound_discharge

First sample discharging a callee `requires` on a bound
learned from another call's closed result:
`s = opaque()` binds 4, so `echo(s)` proves `scale <= 5`
structurally at the instantiated judgment.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Learnings

- `_ = sink(9);` (underscore discard of a call result)
  compiles but warns: discarding a PROVABLY PURE call is
  dead code — no service reach, suspension, blocking, or
  mutable out-parameters.
