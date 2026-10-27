# Private-write soundness controls

The [two-arrival pair](multiple_arrivals/main.omg) has a private receiving helper
called through both branches: replacement is bounded on one and literal 60 on
the other. Its [twin](multiple_arrivals/invalid/main.omg) changes only that second
arrival to 61. Both branches execute in the positive oracle.

The [snapshot pair](snapshot/main.omg) reads `source.value` before a later actual
mutates it to 61. The receiver stores the saved 60; the oracle also checks the
source mutation and untouched nonzero fields before and after the target slot. Its [twin](snapshot/invalid/main.omg)
reverses the two actuals and corresponding formals, so the receiver sees the
later 61. Both twins require the receiving default-domain refusal, not an
unrelated arithmetic or parser error.

These exact existing controls are reused instead of copied into this directory:

- A RHS/prefix write invalidates the referenced bound:
  [prior_write](../../../nested_machine_invocation/private_arithmetic/borrowed/controls/prior_write/main.omg),
  pinned as `fail/facts/nested_machine_private_arithmetic_borrowed_prior_write_rejected`.
- An unformed partial expression cannot establish its own premise:
  [self_formation](../../../nested_machine_invocation/private_arithmetic/feasible/controls/self_formation/main.omg),
  pinned as `fail/facts/nested_machine_private_arithmetic_feasible_self_formation_rejected`.
- An escaping helper cannot infer its bound from a subset of callers:
  [escaping_selection](../../../nested_machine_invocation/private_arithmetic/controls/escaping_selection/main.omg),
  pinned as `fail/facts/nested_machine_private_arithmetic_escaping_selection_rejected`.
- Cyclic arrivals retain their own roots and cannot donate an earlier guard:
  [alternate_arrival](../../../nested_machine_invocation/private_arithmetic/cyclic/controls/alternate_arrival/main.omg)
  and [nested_cycle](../../../nested_machine_invocation/private_arithmetic/cyclic/controls/nested_cycle/main.omg),
  pinned as `fail/facts/nested_machine_private_cyclic_{alternate_arrival,nested_cycle}_rejected`.
  The independent call control `fail/calls/recursive_terminal_machine_precondition`
  additionally rejects a recursive actual violating the fresh invocation's requires.

The four new source/build pairs are pinned as
`data_where_private_write_{multiple_arrivals,snapshot}_{exit,rejected}` under
`tests/omega/{pass,fail}/facts`. Positive authored sources bind all four hosted
entries and declare exit 0. No new recursive inference or language restriction is
introduced by these controls.
