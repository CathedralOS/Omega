# stateful_arm_tail_call

A guarded transition arm's terminal call invokes a multi-state machine that
keeps the borrowed receiver: `Main::check` tail-calls `Main::verify(main,
k_value)` only on its true arm, and `verify` — itself a state machine —
continues with the same `&mut Main` loan plus a plain field-read argument.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
