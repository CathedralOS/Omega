# operand_schedule

Transition subjects and edge operands are scheduled
expressions: `false && clear(&mut flag)` skips the
side effect, `flag && demand(flag)` discharges the
callee's `requires` from the same evaluation, and edge
arguments observe pre-transfer state. Machines whose
entire body is one transition carry `requires`/
`ensures` contracts.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
