# sum_receiver_dispatch

An ordinary static machine may explicitly borrow a sum rather than a record
and dispatch on its cases. The present fixture still spells that operation
`Transition::equals(transition: &Transition, ...)`; that data-owned path is
migration debt under `STATIC-MACHINE-MIGRATION` in
[TASKS.md](../../../../../TASKS.md), not accepted binding of code to data.
Retain the explicit subject, case knowledge and state arguments when migrating.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
