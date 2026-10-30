# machine_param_dispatch

`machine step<T, machine Increment>(subject: &mut T)
requires machine Increment satisfies
Incrementable<T>::increment` — a machine parameter
bound by a trait requirement dispatches statically:
`step<Counter, Counter::increment>` calls it with no
runtime dictionary.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
