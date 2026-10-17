# call_result_bound_discharge

A callee's `requires` discharges on a bound the caller learned from
another call's closed result: the result is 4, so `echo` proves
`scale <= 5` structurally at the instantiated judgment.

Compile-checked only: no standard-library dependency and no
`Main::exit` or `ProcessExit` provider. Each place is an `omega check`
target, not a native run. There is no exit oracle.

This fact is one row of nine places. Each place has one accepting
project and one invalid control. The control is the same place fed a
closed result of 9, so `scale <= 5` does not discharge.

| Place | Project |
| --- | --- |
| local | `local/` |
| field | `field/` |
| parameter | `parameter/` |
| call result | `call_result/` |
| match join | `match_join/` |
| const name | `const_name/` |
| another state | `other_state/` |
| another machine | `other_machine/` |
| another target body | `target_body/` |

## Learnings

- `_ = sink(9);` in `local/` (underscore discard of a call result)
  compiles but warns: discarding a PROVABLY PURE call is
  dead code — no service reach, suspension, blocking, or
  mutable out-parameters.
