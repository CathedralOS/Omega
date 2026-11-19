# suspend_axis

`suspends` declared machine axis + `suspend scheduler.park()` call site: a borrow-carrying record (`Message{body: &Cell}`) formed before the suspension stays readable; `Build::select_provider<Scheduler, InlineScheduler>` provisions the Binding field. Learning: suspend must be the LAST use of `main` — its default carry policy is `suspension: forbidden` for live values.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
