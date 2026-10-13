# identity_measure_widths

The identity measure `value -> value` is definable at every width:
the same `measure Countdown::Remaining(value: T) -> T { value }`
shape composes the rank over `u32` and `u64` subjects alike.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
