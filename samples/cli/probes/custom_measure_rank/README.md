# custom_measure_rank

`measure Card::PowerOrder(power: u64) -> u64 { power }` — an authored
ranking order; `terminates by power -> Card::PowerOrder` ranks the
recursive call through it instead of the default order.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
