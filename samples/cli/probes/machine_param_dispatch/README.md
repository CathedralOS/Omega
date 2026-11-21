# machine_param_dispatch

`step<T, const Increment: Incrementable<T>::increment>` accepts a
selected implementation. The call `step::<Counter, Counter::increment>`
passes the counter to that implementation.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
